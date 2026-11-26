# Synapse & CortexKit LLM Integration Guide for Rust Applications

This document provides architectural guidance, wire contracts, and concrete Rust code examples for external applications (such as `314track` / `uc-sound`) consuming CortexKit capabilities and local Synapse inference over the SubC loopback daemon.

---

## 1. System Architecture: SubC as an IPC Daemon

CortexKit supervised modules (including `ck-synapse`, `ck-claustrum`, `ck-fusiform`, `ck-entorhinal`, and `ck-uc-discussions`) do not require in-process linking. Instead, they run as standalone daemon-supervised processes communicating over SubC loopback TCP (`127.0.0.1`) authenticated via per-session HMAC handshake nonces.

```
+-------------------------------------------------------+
|  External Rust Application (e.g. 314track / uc-sound) |
+-------------------------------------------------------+
          │
          │ subc-client-rs (SubcConsumer)
          ▼
+───────────────────────────────────────────────────────+
|               ck-subc daemon (Loopback TCP)           |
+───────────────────────────────────────────────────────+
       │                             │
       ▼ RouteTarget::               ▼ RouteTarget::
         ManagementSurface             ManagementSurface
+───────────────────────────+   +───────────────────────+
|   ck-synapse              |   |   ck-claustrum        |
|  - embed.batch / query    |   |  - vault credentials  |
|  - rerank.score           |   |  - OAuth/API tokens   |
|  - microllm.oneshot       |   +───────────────────────+
|  - owned_decode.*         |
+───────────────────────────+
```

Any outside Rust application can interact with any supervised CortexKit service by using the published crates on [crates.io](https://crates.io).

---

## 2. Published Crates on crates.io

All core crates required to connect to the daemon, authenticate, and call module operations are published and versioned on crates.io:

### A. SubC Daemon IPC & Routing

| Crate | Published Version | Role & Usage |
|---|---|---|
| **`subc-client-rs`** | `0.41.14` | High-level consumer and module SDK. Provides `SubcConsumer::connect`, candidate connection path auto-discovery (`subc_client_rs::discovery_candidates`), call options, and connection pooling. |
| **`subc-protocol`** | `0.37.9` | Fixed 21-byte binary envelope, `BindIdentity`, `RouteTarget`, `CallError`, and channel frame definitions. |
| **`subc-transport`** | `0.18.2` | Pre-envelope authentication handshake, loopback TCP client transport, and connection-file resolution. |
| **`subc-control`** | `0.42.10` | Client-facing control-plane types (`route.bind`, `route.open`, `health.check`). |

### B. Credential Management & Remote LLMs (Claustrum)

If the Rust application needs to invoke frontier commercial LLMs (Anthropic Claude Opus/Sonnet, OpenAI GPT-6, Google Gemini) to decompose musical briefs, generate chord progressions, or produce ABC notation:

| Crate | Published Version | Role & Usage |
|---|---|---|
| **`cortexkit-claustrum-client`** | `0.16.0` | Client library for the `ck-claustrum` credential vault. Fetches short-lived access tokens, bearer handles, and API keys without persisting secrets in project configuration files. |

### C. LLM Tooling, Tokens & Batch Processing

| Crate | Published Version | Role & Usage |
|---|---|---|
| **`cortexkit-tokens`** | `0.2.0` | Fast offline BPE tokenizer and token-budget estimator for prompt sizing. |
| **`cortexkit-role-llm-runner`** | `0.9.7` | Wire contracts and transcript data models for CortexKit LLM runners. |
| **`cortexkit-llm-tools`** | `0.4.0` | Schemas and validation types for structured single-call tool invocations. |
| **`cortexkit-model-batch`** | `0.3.4` | Slicing rules, error models, and conservative cost calculations for batch inference. |
| **`cortexkit-role-classifier`** | `0.6.5` | Wire contracts for zero-shot text and prompt classification. |
| **`cortexkit-role-extractor`** | `0.3.0` | Wire contracts for structured entity extraction from unstructured text. |

---

## 3. What Synapse Provides (and What It Does Not)

### A. Supported Operations in Synapse (`module_id: "synapse"`)

Synapse exposes a `ManagementSurface` role with the following verified operations:

1. **`embed.batch` / `embed.query`**:
   * **Active model**: `gte-modernbert-base-f16` (768 dimensions, 8,192 token window).
   * **Hardware Engine**: Apple Silicon Metal MPSGraph (`owned-metal-v1`).
   * **Measured performance**: ~15 ms p50 latency, ~4,245 tokens/sec.
   * **Application**: Vector indexing and semantic retrieval over musical motif metadata, harmonic descriptions, and textual director briefs.
2. **`rerank.score`**:
   * **Active model**: `gte-reranker-modernbert-base-f32` (cross-encoder).
   * **Measured performance**: ~23 ms single-item p50 latency.
   * **Application**: Fine-grained relevance scoring to match candidate leitmotifs against complex narrative requirements.
3. **`microllm.oneshot` & `owned_decode.*`**:
   * **Models**: Local small language models managed by worker sub-processes (`ck-synapse-worker-llama`, etc.).
   * **Application**: Offline local extraction of structured musical parameters from natural-language prompts.

### B. What Synapse Does NOT Provide (Audio Foundation Models)

Synapse is built for text representations (embeddings, cross-encoders, and text LLMs). **It does not contain audio generation models, spectrogram VAEs, or music diffusion weights** (e.g. YuE2, MiniMax-Music3, MusicGen).

* **Recommendation for Audio Models**: 314track's design—retaining neural audio generators (YuE2, MiniMax) as Python MLX worker processes running in their respective `.venv` environments and supervised by the Rust host over length-delimited stdio—is the optimal architecture. Porting YuE2/MiniMax to native Rust is unnecessary and would require implementing custom Apple Silicon Metal kernels (e.g. Snake activations, ODE solvers, and multi-codebook RVQ).

---

## 4. Implementation Example: Consuming Synapse from Rust

Below is an end-to-end, reproducible Rust module demonstrating how an outside application connects to the running SubC daemon and invokes `embed.batch` on Synapse.

### `Cargo.toml`

```toml
[dependencies]
subc-client-rs = "0.41"
subc-protocol = "0.37"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### `src/synapse.rs`

```rust
use std::time::Duration;
use serde::{Deserialize, Serialize};
use subc_client_rs::consumer::{CallOptions, ConsumerOptions, SubcConsumer};
use subc_protocol::{BindIdentity, RouteTarget};

#[derive(Debug, Deserialize)]
pub struct EmbedResult {
    pub dims: usize,
    pub vectors: Vec<VectorItem>,
}

#[derive(Debug, Deserialize)]
pub struct VectorItem {
    pub id: String,
    pub vector: Vec<f32>,
}

pub struct SynapseClient {
    consumer: SubcConsumer,
}

impl SynapseClient {
    /// Connect to the live SubC daemon using standard connection file discovery.
    pub async fn connect() -> Result<Self, Box<dyn std::error::Error>> {
        let candidates = subc_client_rs::discovery_candidates(
            None,
            std::env::var_os("SUBC_CONNECTION_FILE").as_deref(),
        );

        let mut consumer = None;
        for path in &candidates {
            if let Ok(c) = SubcConsumer::connect(path, ConsumerOptions::default()).await {
                consumer = Some(c);
                break;
            }
        }

        let consumer = consumer.ok_or_else(|| {
            format!(
                "Could not locate active SubC connection file across {} candidate paths",
                candidates.len()
            )
        })?;

        Ok(Self { consumer })
    }

    /// Request 768-dimensional normalized text embeddings from Synapse via Metal MPSGraph.
    pub async fn embed_texts(
        &self,
        texts: Vec<String>,
        is_query: bool,
    ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        let identity = BindIdentity::new("314track-music", "motif-embedding");

        let input_type = if is_query { "query" } else { "document" };
        let request_payload = serde_json::to_vec(&serde_json::json!({
            "method": "embed.batch",
            "params": {
                "model": "gte-modernbert-base-f16",
                "input_type": input_type,
                "texts": texts
            }
        }))?;

        let options = CallOptions {
            timeout: Duration::from_secs(10),
            ..Default::default()
        };

        let raw_reply = self
            .consumer
            .call(
                RouteTarget::ManagementSurface {
                    module_id: "synapse".to_string(),
                },
                identity,
                request_payload,
                options,
            )
            .await?;

        let parsed: serde_json::Value = serde_json::from_slice(&raw_reply)?;
        
        if let Some(err) = parsed.get("error") {
            return Err(format!("Synapse error: {err}").into());
        }

        let embed_res: EmbedResult = serde_json::from_value(parsed["result"].clone())?;
        Ok(embed_res.vectors.into_iter().map(|v| v.vector).collect())
    }

    pub async fn close(self) {
        self.consumer.close().await;
    }
}
```

---

## 5. Architectural Summary for 314track

1. **Keep Audio MLX Workers as Planned**: Do not attempt to force YuE2/MiniMax into Synapse or pure Rust. Keep them as supervised Python processes over length-delimited stdio.
2. **Adopt `subc-client-rs` in `314track`**: Use it to tap into the local daemon for `synapse` (embeddings + reranking for motif search) and `claustrum` (API keys/OAuth tokens for remote LLMs).
3. **Incorporate Published Role Crates**: Add `cortexkit-tokens` for token budgeting and `cortexkit-claustrum-client` whenever frontier models are needed for symbolic composition.
