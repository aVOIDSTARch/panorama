# Cerebro — Knowledge Graph System Design

> Personal knowledge graph with multi-modal epistemic tracking:
> empirical facts, hypotheses, and fictional fabrications in a single traversable graph.

---

## 1. Data Model

### 1.1 Node Schema

Every entity in the graph carries:

```json
{
  "id":               "node:uuid-or-slug",
  "label":            "Marie Curie",
  "type":             "Person",
  "aliases":          ["M. Curie", "Madame Curie"],
  "epistemic_mode":   "empirical",
  "fictional_world":  null,
  "created_at":       "2026-03-18",
  "notes":            "free text"
}
```

`epistemic_mode` for nodes: `empirical | hypothetical | fictional`

For fictional nodes, `fictional_world` names the constructed world (e.g. `"world:project_lazarus"`).
Speculative entities that may not exist get `epistemic_mode: hypothetical`.

---

### 1.2 Edge Schema

The edge is the primary unit of epistemic annotation:

```json
{
  "id":               "edge:uuid",
  "subject":          "node:marie-curie",
  "predicate":        "influenced",
  "object":           "node:nietzsche",

  "epistemic_mode":   "hypothetical",
  "confidence":       "plausible",
  "fictional_world":  null,

  "source":           "self",
  "evidence":         "Nietzsche owned a copy of OotOS — letter to Rée, 1875",
  "created_at":       "2026-03-18",
  "updated_at":       "2026-03-18"
}
```

#### `epistemic_mode` (enum)

| Value          | Meaning                                              |
|----------------|------------------------------------------------------|
| `empirical`    | Attempting truth — subject to confidence rating      |
| `hypothetical` | Attempting truth but unresolved — confidence applies |
| `fictional`    | Not attempting truth — internal consistency applies  |

#### `confidence` (enum — empirical and hypothetical only)

| Value         | Meaning                                        |
|---------------|------------------------------------------------|
| `axiomatic`   | Definitional, not subject to revision          |
| `established` | Consensus or well-cited                        |
| `probable`    | Strong evidence, not definitively settled      |
| `plausible`   | Some evidence, contested or incomplete         |
| `speculative` | Intuition or raw hypothesis, no hard evidence  |
| `null`        | Inapplicable — used for all fictional edges    |

#### `source` (enum)

| Value      | Meaning                                         |
|------------|-------------------------------------------------|
| `self`     | Your own assertion or hypothesis                |
| `citation` | Derived from an external source                 |
| `inferred` | Machine-derived — quarantined by default        |

> **Rule:** Machine-inferred edges default to `confidence: speculative` and `source: inferred`
> regardless of model confidence. Never allow inferred edges to promote themselves.

---

### 1.3 Junction Nodes

A node is a **junction** when it holds edges in more than one `epistemic_mode`.
Example: `node:vienna-1900` has empirical edges to Freud and Wittgenstein,
and fictional edges to a character who lives there.

Query for junction nodes to find where your research and fiction intersect:

```cypher
MATCH (n)
WHERE size([(n)-[r]-() | r.epistemic_mode]) > 1
  AND 'empirical' IN [(n)-[r]-() | r.epistemic_mode]
  AND 'fictional' IN [(n)-[r]-() | r.epistemic_mode]
RETURN n.label, n.id
```

---

## 2. System Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Ingestion layer                    │
│   NER · entity resolution · deduplication          │
│   (spaCy / custom scripts)                         │
└────────────────┬────────────────────────────────────┘
                 │
       ┌─────────┴──────────┐
       ▼                    ▼
┌─────────────┐      ┌─────────────────┐
│ Graph store │      │  Full-text index │
│  (Kùzu)     │      │ (Meilisearch)   │
└──────┬──────┘      └───────┬─────────┘
       │                     │
       ▼                     ▼
┌─────────────┐      ┌─────────────────┐
│Vector index │      │  Entity linker  │
│(pgvector or │      │  string → node  │
│ Chroma)     │      │  UUID           │
└──────┬──────┘      └───────┬─────────┘
       └──────────┬──────────┘
                  ▼
        ┌──────────────────┐
        │   Query layer    │
        │  Cypher + hybrid │
        │  vector search   │
        └──────────────────┘
```

---

## 3. Primary Recommendation: Kùzu + Meilisearch + Chroma

### Rationale

This stack is the strongest fit for a homelab-scale personal knowledge system
with complex epistemic metadata, Cypher query familiarity, and a privacy-first,
no-cloud mandate.

### 3.1 Graph Store — Kùzu

- **What it is:** Embedded graph database (C++ core, Python/Node bindings).
  Think DuckDB but for property graphs. No server process — runs in-process.
- **Query language:** Cypher (openCypher subset)
- **Why it fits Cerebro:**
  - Zero-server deployment — runs directly inside your Python backend
  - Columnar storage engine gives fast aggregate queries across epistemic_mode flags
  - Property graph natively supports the full edge schema above
  - ACID transactions — safe for incremental ingestion
  - Active development, MIT-licensed
- **Limitation:** No built-in full-text search; no vector search natively.
  Both are handled by companion services.

**Installation:**
```bash
pip install kuzu
```

**Schema definition:**
```python
import kuzu

db = kuzu.Database("./cerebro.db")
conn = kuzu.Connection(db)

conn.execute("""
  CREATE NODE TABLE Entity(
    id STRING PRIMARY KEY,
    label STRING,
    type STRING,
    epistemic_mode STRING,
    fictional_world STRING,
    aliases STRING[],
    created_at STRING,
    notes STRING
  )
""")

conn.execute("""
  CREATE REL TABLE Assertion(
    FROM Entity TO Entity,
    id STRING,
    predicate STRING,
    epistemic_mode STRING,
    confidence STRING,
    fictional_world STRING,
    source STRING,
    evidence STRING,
    created_at STRING,
    updated_at STRING
  )
""")
```

**Junction node query:**
```python
result = conn.execute("""
  MATCH (a:Entity)-[r:Assertion]->(b:Entity)
  WITH a, collect(DISTINCT r.epistemic_mode) AS modes
  WHERE 'empirical' IN modes AND 'fictional' IN modes
  RETURN a.label, a.id, modes
""")
```

---

### 3.2 Full-text / Entity Search — Meilisearch

- **What it is:** Open-source search engine written in Rust.
  Extremely fast, typo-tolerant, self-hosted.
- **Why it fits Cerebro:**
  - String → node UUID disambiguation (the entity linking problem)
  - Searches across labels, aliases, and notes fields
  - Faceted filtering by `epistemic_mode`, `type`, `fictional_world`
  - REST API — trivial to integrate with any backend
  - Single binary, ~50MB RAM at idle — homelab-friendly
- **Limitation:** Not a graph store; does not understand relationships.

**Docker deployment:**
```bash
docker run -d \
  --name meilisearch \
  -p 7700:7700 \
  -v $(pwd)/meili_data:/meili_data \
  getmeili/meilisearch:latest \
  meilisearch --master-key="your-master-key"
```

**Index configuration:**
```python
import meilisearch

client = meilisearch.Client("http://localhost:7700", "your-master-key")
index = client.index("cerebro_entities")

index.update_filterable_attributes(["epistemic_mode", "type", "fictional_world"])
index.update_searchable_attributes(["label", "aliases", "notes"])
index.update_ranking_rules([
  "words", "typo", "proximity", "attribute", "sort", "exactness"
])
```

---

### 3.3 Vector / Semantic Search — Chroma

- **What it is:** Open-source embedding database, Python-native.
- **Why it fits Cerebro:**
  - Stores entity embeddings (run through a local model like
    `sentence-transformers/all-MiniLM-L6-v2`) for semantic similarity search
  - "Find nodes semantically similar to this concept" without exact string match
  - Persistent local storage, no server required in embedded mode
  - Metadata filtering — query only `epistemic_mode: empirical` vectors
- **Limitation:** Not a substitute for graph traversal.
  Complements Kùzu, does not replace it.

**Setup:**
```python
import chromadb
from sentence_transformers import SentenceTransformer

chroma = chromadb.PersistentClient(path="./chroma_data")
collection = chroma.get_or_create_collection("cerebro_nodes")

model = SentenceTransformer("all-MiniLM-L6-v2")

def index_node(node: dict):
    embedding = model.encode(f"{node['label']} {node['notes']}").tolist()
    collection.add(
        ids=[node["id"]],
        embeddings=[embedding],
        metadatas=[{
            "epistemic_mode": node["epistemic_mode"],
            "type":           node["type"],
            "fictional_world": node.get("fictional_world", "")
        }]
    )

def semantic_search(query: str, epistemic_mode: str = None, n: int = 10):
    where = {"epistemic_mode": epistemic_mode} if epistemic_mode else None
    embedding = model.encode(query).tolist()
    return collection.query(
        query_embeddings=[embedding],
        n_results=n,
        where=where
    )
```

---

### 3.4 Primary Stack Summary

| Layer          | Software       | License | RAM at idle | Notes                       |
|----------------|----------------|---------|-------------|------------------------------|
| Graph store    | Kùzu           | MIT     | ~30MB       | Embedded, no server          |
| Full-text      | Meilisearch    | MIT     | ~50MB       | Single Docker container      |
| Vector search  | Chroma         | Apache  | ~40MB       | Embedded or Docker           |
| Embeddings     | sentence-transformers | Apache | on-demand | Local CPU inference fine |
| Backend glue   | Python 3.11+   | PSF     | —           | FastAPI or Flask             |

**Total overhead:** ~120MB RAM at idle. Appropriate for `ubuntu-server1` alongside Alexandria.

---

## 4. Alternative A: Apache Jena TDB2 + Fuseki (RDF/SPARQL stack)

### When to choose this over Kùzu

If you anticipate wanting to interoperate with external knowledge bases
(Wikidata, DBpedia, schema.org), export to standard formats, or query
with the expressive power of SPARQL's graph pattern matching,
Jena is the more principled choice. It is heavier and more operationally demanding.

### Components

**Apache Jena TDB2** — native RDF triple store with persistent disk-backed storage.
All six SPO permutation indexes built automatically.

**Apache Fuseki** — SPARQL 1.1 endpoint that sits in front of TDB2.
HTTP API for queries and updates.

### Trade-offs vs. primary recommendation

| Dimension       | Kùzu stack                  | Jena/Fuseki                        |
|-----------------|-----------------------------|------------------------------------|
| Query language  | Cypher (ergonomic)          | SPARQL (more expressive, verbose)  |
| Interoperability| None natively               | Full RDF/OWL/SPARQL standard       |
| Deployment      | Embedded, zero config       | JVM process, ~300MB RAM            |
| Schema          | Property graph (flexible)   | RDF (everything is a triple)       |
| Epistemic props | Edge properties directly    | Requires reification (verbose)     |
| Homelab fit     | Excellent                   | Acceptable — JVM overhead notable  |

### Epistemic metadata in RDF requires reification

In RDF, to annotate an edge with confidence, you must turn the triple into a node:

```turtle
:assertion1 a rdf:Statement ;
  rdf:subject    :marie-curie ;
  rdf:predicate  :influenced ;
  rdf:object     :nietzsche ;
  cerebro:epistemicMode   "hypothetical" ;
  cerebro:confidence      "plausible" ;
  cerebro:evidence        "Letter to Rée, 1875" .
```

This is functionally correct but syntactically heavy.
RDF-star (supported in Jena 4.x) cleans this up significantly:

```turtle
<< :marie-curie :influenced :nietzsche >>
  cerebro:epistemicMode "hypothetical" ;
  cerebro:confidence    "plausible" .
```

**Verdict on Alternative A:** Choose Jena if interoperability with external
RDF datasets is a priority. Accept the JVM overhead and SPARQL verbosity
as the cost. For a self-contained personal system, it is over-engineered.

---

## 5. Alternative B: Neo4j Community Edition

### When to consider this

Neo4j is the most mature native graph database available and has the largest
ecosystem of tooling, documentation, and community resources.
If operational maturity and GUI tooling (Neo4j Browser, Bloom) matter more
than license purity and resource footprint, it is worth considering.

### Trade-offs vs. primary recommendation

| Dimension        | Kùzu stack               | Neo4j Community              |
|------------------|--------------------------|------------------------------|
| License          | MIT                      | GPL-3 (Community) — watch this |
| RAM              | ~30MB                    | ~512MB minimum, 1GB+ typical |
| Query language   | Cypher                   | Cypher (same language)       |
| Clustering       | Not supported            | Not in Community edition     |
| GUI tooling      | None built-in            | Neo4j Browser included       |
| APOC procedures  | Not available            | Rich plugin ecosystem        |
| Embedded mode    | Yes                      | No — separate server process |

### Hard limitation

Neo4j Community Edition does not allow you to run it as a service within
a commercial product or SaaS offering. For a personal homelab system,
GPL-3 is not a practical concern — but note it if Cerebro ever grows
into something you share or distribute.

**Verdict on Alternative B:** Use Neo4j if you want mature GUI tooling
and the same Cypher you'd use in Kùzu. The resource overhead and license
ambiguity make it a weaker fit for a privacy-first homelab, but it is not
a wrong choice.

---

## 6. Alternative C: PostgreSQL + AGE extension

### When to consider this

If you are already running PostgreSQL for Alexandria or another service,
Apache AGE gives you graph query capabilities (openCypher) on top of
your existing Postgres instance. One server, one backup regime,
one operational surface.

### Trade-offs

| Dimension        | Kùzu stack               | PostgreSQL + AGE             |
|------------------|--------------------------|------------------------------|
| Deployment       | Embedded Python lib      | Existing Postgres instance   |
| Graph performance| Native graph engine      | Graph on top of RDBMS        |
| Multi-hop queries| Fast (native adjacency)  | Slower at depth (join-based) |
| Full-text search | Separate (Meilisearch)   | pgvector + tsvector built-in |
| Vector search    | Separate (Chroma)        | pgvector built-in            |
| Operational cost | Low (new process)        | Zero (extends existing DB)   |

### When it breaks down

AGE graph traversal degrades noticeably past 3-4 hops because it translates
Cypher to SQL joins internally. For a knowledge graph where multi-hop
traversal ("find all entities connected to this fictional character through
empirical junction nodes") is a core query pattern, this is a real cost.

**Verdict on Alternative C:** Compelling if you are already running Postgres
and your graph queries are predominantly 1-2 hops. A pragmatic shortcut,
not the principled choice.

---

## 7. Recommended Deployment on ubuntu-server1

```
/srv/cerebro/
├── cerebro.db/           # Kùzu database files
├── chroma_data/          # Chroma vector store
├── meili_data/           # Meilisearch index (Docker volume)
├── ingest/               # Scripts for adding nodes/edges
│   ├── add_entity.py
│   ├── add_assertion.py
│   └── resolve_entity.py # string → UUID via Meilisearch
├── api/                  # FastAPI backend (optional)
│   └── main.py
└── docker-compose.yml    # Meilisearch only
```

**docker-compose.yml:**
```yaml
services:
  meilisearch:
    image: getmeili/meilisearch:latest
    ports:
      - "7700:7700"
    volumes:
      - ./meili_data:/meili_data
    environment:
      MEILI_MASTER_KEY: "your-master-key"
    restart: unless-stopped
```

---

## 8. Decision Matrix

| Criterion                            | Kùzu stack | Jena/Fuseki | Neo4j CE | PG + AGE |
|--------------------------------------|------------|-------------|----------|----------|
| Homelab resource footprint           | ★★★★★      | ★★★         | ★★       | ★★★★     |
| Epistemic metadata ergonomics        | ★★★★★      | ★★★         | ★★★★★    | ★★★★     |
| Multi-hop traversal performance      | ★★★★       | ★★★★        | ★★★★★    | ★★       |
| RDF interoperability                 | ✗          | ★★★★★       | ✗        | ✗        |
| License clarity                      | ★★★★★      | ★★★★★       | ★★★      | ★★★★★    |
| Operational simplicity               | ★★★★★      | ★★★         | ★★★      | ★★★★     |
| Cypher query language                | ✓          | ✗           | ✓        | ✓        |
| GUI tooling                          | ✗          | Fuseki UI   | ★★★★★    | pgAdmin  |

**Primary recommendation: Kùzu + Meilisearch + Chroma**

---

## 9. One Firm Warning on Inference

If you later integrate an LLM to suggest edges or derive relationships,
enforce a hard provenance wall:

1. All LLM-derived edges get `source: "inferred"` and `confidence: "speculative"`
   **regardless** of model confidence score.
2. Inferred edges live in a quarantine state until you manually promote them.
3. Promotion requires explicit human action — an `approve_assertion(edge_id)` function
   that upgrades `source` to `self` and lets you set a real confidence level.

Models confabulate with high stated confidence. If inferred edges mix freely
with hand-authored ones, the epistemic integrity of Cerebro degrades silently.
The quarantine wall is not optional.

---

*Document version: 2026-03-18 | System: Cerebro | Author: Louie*
