# Cerebro — Backend Implementation Plan

> Step-by-step implementation of the primary stack recommendation from
> `cerebro-kg-design.md` using TypeScript throughout.
> Stack: Kùzu (graph) · Meilisearch (full-text) · Chroma (vector) · Fastify (API)

---

## Language Decision: TypeScript over Rust

Kùzu ships official Node.js bindings (`kuzu` npm package, v0.11.3).
Chroma and Meilisearch both have first-class TypeScript SDKs.
The full primary stack is expressible in TypeScript without wrapping
C bindings or maintaining FFI glue.

Rust is the better choice if you later need to embed Cerebro inside
another Rust process or require sub-millisecond response times.
For a homelab API server handling personal query volumes, TypeScript
on Node.js is the pragmatic winner — strong typing, mature ecosystem,
no meaningful performance penalty at this scale.

---

## Repository Layout

```
/srv/cerebro/
├── package.json
├── tsconfig.json
├── docker-compose.yml          # Meilisearch only
├── cerebro.db/                 # Kùzu data files (created at runtime)
├── chroma_data/                # Chroma persistent store
├── meili_data/                 # Meilisearch index (Docker volume)
├── src/
│   ├── types.ts                # All shared types and enums
│   ├── db/
│   │   ├── graph.ts            # Kùzu connection and schema bootstrap
│   │   ├── search.ts           # Meilisearch client and index config
│   │   └── vectors.ts          # Chroma client and embedding helpers
│   ├── graph/
│   │   ├── nodes.ts            # Entity CRUD
│   │   ├── edges.ts            # Assertion CRUD
│   │   ├── sources.ts          # Source node CRUD
│   │   ├── citations.ts        # CitedBy relationship CRUD
│   │   └── queries.ts          # Junction queries, path queries
│   ├── quarantine/
│   │   ├── schema.ts           # SQLite quarantine table
│   │   ├── ingest.ts           # Write candidates to quarantine
│   │   └── promote.ts          # Promote approved candidates to graph
│   ├── api/
│   │   ├── server.ts           # Fastify server entry point
│   │   ├── routes/
│   │   │   ├── entities.ts
│   │   │   ├── assertions.ts
│   │   │   ├── search.ts
│   │   │   └── quarantine.ts
│   └── validate/
│       └── integrity.ts        # Citation integrity checks
└── scripts/
    └── bootstrap.ts            # One-time schema creation
```

---

## Phase 1 — Project Scaffold

### Step 1.1 — Initialize the project

```bash
mkdir -p /srv/cerebro/src/{db,graph,quarantine,api/routes,validate}
mkdir -p /srv/cerebro/scripts
cd /srv/cerebro

npm init -y
npm install typescript tsx @types/node --save-dev
npm install kuzu meilisearch chromadb better-sqlite3 @types/better-sqlite3
npm install fastify @fastify/cors zod
npm install @xenova/transformers
```

### Step 1.2 — `tsconfig.json`

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "Node16",
    "moduleResolution": "Node16",
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true
  },
  "include": ["src/**/*", "scripts/**/*"]
}
```

### Step 1.3 — `package.json` scripts

```json
{
  "scripts": {
    "bootstrap": "tsx scripts/bootstrap.ts",
    "dev":       "tsx watch src/api/server.ts",
    "build":     "tsc",
    "start":     "node dist/api/server.js"
  }
}
```

---

## Phase 2 — Shared Types

### Step 2.1 — `src/types.ts`

All enums and interfaces used across every module. Define once, import everywhere.

```typescript
export type EpistemicMode = "empirical" | "hypothetical" | "fictional";

export type Confidence =
  | "axiomatic"
  | "established"
  | "probable"
  | "plausible"
  | "speculative"
  | null; // null = fictional edges only

export type AssertionSource = "self" | "citation" | "inferred";

export type CitationType = "direct" | "indirect" | "refuting" | "contextual";

export type ReliabilityTier = "primary" | "secondary" | "tertiary" | "grey";

export type SourceType =
  | "primary_research"
  | "systematic_review"
  | "encyclopedia"
  | "reference_work"
  | "official_record"
  | "contemporaneous"
  | "monograph"
  | "grey";

// ── Node ────────────────────────────────────────────────────────────────────

export interface CerebroEntity {
  id:             string;         // "node:<uuid>"
  label:          string;
  type:           string;
  aliases:        string[];
  epistemic_mode: EpistemicMode;
  fictional_world: string | null;
  created_at:     string;         // ISO date
  notes:          string | null;
}

// ── Edge ────────────────────────────────────────────────────────────────────

export interface CerebroAssertion {
  id:             string;         // "edge:<uuid>"
  subject_id:     string;
  predicate:      string;
  object_id:      string;
  epistemic_mode: EpistemicMode;
  confidence:     Confidence;
  fictional_world: string | null;
  source:         AssertionSource;
  evidence:       string | null;
  created_at:     string;
  updated_at:     string;
}

// ── Source node ──────────────────────────────────────────────────────────────

export interface CerebroSource {
  id:               string;       // "node:source:<uuid>"
  label:            string;
  source_type:      SourceType;
  reliability_tier: ReliabilityTier;
  uri:              string | null;
  doi:              string | null;
  isbn:             string | null;
  authors:          string[];
  publication_year: number | null;
  publisher:        string | null;
  journal:          string | null;
  peer_reviewed:    boolean;
  retracted:        boolean;
  retraction_uri:   string | null;
  accessed_at:      string;
  added_at:         string;
  notes:            string | null;
}

// ── Citation relationship ────────────────────────────────────────────────────

export interface CerebroCitation {
  id:              string;        // "citation:<uuid>"
  assertion_id:    string;
  source_id:       string;
  citation_type:   CitationType;
  page_or_section: string | null;
  quote:           string | null;
  added_at:        string;
}

// ── Quarantine candidate ─────────────────────────────────────────────────────

export type CandidateStatus =
  | "pending"
  | "approved"
  | "edited"
  | "rejected"
  | "promoted";

export interface ExtractionCandidate {
  id:                    string;
  subject_label:         string;
  subject_node_id:       string | null;
  predicate:             string;
  object_label:          string;
  object_node_id:        string | null;
  raw_sentence:          string;
  source_paper_uri:      string;
  source_section:        string | null;
  page_number:           number | null;
  hedge_flag:            boolean;
  hedge_text:            string | null;
  scope_qualifier:       string | null;
  negation_flag:         boolean;
  suggested_confidence:  Confidence;
  extractor_model:       string;
  extraction_method:     string;
  status:                CandidateStatus;
  final_confidence:      Confidence | null;
  final_subject_label:   string | null;
  final_predicate:       string | null;
  final_object_label:    string | null;
  epistemic_mode:        EpistemicMode;
  fictional_world:       string | null;
  reviewer_notes:        string | null;
  source_reliability_tier: ReliabilityTier | null;
  citation_type:         CitationType;
  source_peer_reviewed:  boolean | null;
  source_retracted:      boolean;
  extracted_at:          string;
  reviewed_at:           string | null;
  promoted_edge_id:      string | null;
}
```

---

## Phase 3 — Database Layer

### Step 3.1 — Kùzu graph store — `src/db/graph.ts`

```typescript
import kuzu from "kuzu";
import path from "path";

const DB_PATH = process.env.KUZU_PATH ?? "/srv/cerebro/cerebro.db";

let _db: kuzu.Database | null = null;
let _conn: kuzu.Connection | null = null;

export function getConnection(): kuzu.Connection {
  if (!_conn) {
    _db   = new kuzu.Database(DB_PATH);
    _conn = new kuzu.Connection(_db);
  }
  return _conn;
}

export async function bootstrapSchema(): Promise<void> {
  const conn = getConnection();

  await conn.execute(`
    CREATE NODE TABLE IF NOT EXISTS Entity(
      id             STRING PRIMARY KEY,
      label          STRING,
      type           STRING,
      epistemic_mode STRING,
      fictional_world STRING,
      aliases        STRING[],
      created_at     STRING,
      notes          STRING
    )
  `);

  await conn.execute(`
    CREATE NODE TABLE IF NOT EXISTS Source(
      id               STRING PRIMARY KEY,
      label            STRING,
      source_type      STRING,
      reliability_tier STRING,
      uri              STRING,
      doi              STRING,
      isbn             STRING,
      authors          STRING[],
      publication_year INT64,
      publisher        STRING,
      journal          STRING,
      peer_reviewed    BOOLEAN,
      retracted        BOOLEAN DEFAULT false,
      retraction_uri   STRING,
      accessed_at      STRING,
      added_at         STRING,
      notes            STRING
    )
  `);

  await conn.execute(`
    CREATE REL TABLE IF NOT EXISTS Assertion(
      FROM Entity TO Entity,
      id             STRING,
      predicate      STRING,
      epistemic_mode STRING,
      confidence     STRING,
      fictional_world STRING,
      source         STRING,
      evidence       STRING,
      created_at     STRING,
      updated_at     STRING
    )
  `);

  await conn.execute(`
    CREATE REL TABLE IF NOT EXISTS CitedBy(
      FROM Entity TO Source,
      id              STRING,
      assertion_id    STRING,
      citation_type   STRING,
      page_or_section STRING,
      quote           STRING,
      added_at        STRING
    )
  `);

  console.log("Kùzu schema bootstrapped.");
}
```

> **Note on CitedBy:** Kùzu relationship tables connect node types.
> Since Assertions are edges (not nodes) in this schema, the `CitedBy`
> relationship is attached via `assertion_id` stored as a property, and
> queried by joining through that field. If you later need to reify
> assertions as nodes for richer traversal, that migration is straightforward.

---

### Step 3.2 — Meilisearch client — `src/db/search.ts`

```typescript
import { MeiliSearch } from "meilisearch";

const MEILI_HOST   = process.env.MEILI_HOST   ?? "http://localhost:7700";
const MEILI_KEY    = process.env.MEILI_KEY    ?? "";
const INDEX_NAME   = "cerebro_entities";

export const meili = new MeiliSearch({ host: MEILI_HOST, apiKey: MEILI_KEY });

export async function bootstrapSearchIndex(): Promise<void> {
  const index = meili.index(INDEX_NAME);

  await index.updateFilterableAttributes([
    "epistemic_mode",
    "type",
    "fictional_world",
    "retracted",
  ]);

  await index.updateSearchableAttributes([
    "label",
    "aliases",
    "notes",
  ]);

  await index.updateRankingRules([
    "words", "typo", "proximity", "attribute", "sort", "exactness",
  ]);

  console.log("Meilisearch index configured.");
}

export async function indexEntity(entity: {
  id:             string;
  label:          string;
  aliases:        string[];
  type:           string;
  epistemic_mode: string;
  fictional_world: string | null;
  notes:          string | null;
}): Promise<void> {
  await meili.index(INDEX_NAME).addDocuments([entity]);
}

export async function searchEntities(
  query:          string,
  epistemicMode?: string,
  fictionalWorld?: string,
  limit           = 20,
) {
  const filter: string[] = [];
  if (epistemicMode)  filter.push(`epistemic_mode = "${epistemicMode}"`);
  if (fictionalWorld) filter.push(`fictional_world = "${fictionalWorld}"`);

  return meili.index(INDEX_NAME).search(query, {
    limit,
    filter: filter.length ? filter.join(" AND ") : undefined,
  });
}
```

---

### Step 3.3 — Chroma vector store — `src/db/vectors.ts`

```typescript
import { ChromaClient, Collection } from "chromadb";
import { pipeline, FeatureExtractionPipeline } from "@xenova/transformers";

const CHROMA_PATH = process.env.CHROMA_PATH ?? "http://localhost:8000";
const MODEL_NAME  = "Xenova/all-MiniLM-L6-v2";

const chroma = new ChromaClient({ path: CHROMA_PATH });
let _collection: Collection | null = null;
let _embedder:   FeatureExtractionPipeline | null = null;

async function getEmbedder(): Promise<FeatureExtractionPipeline> {
  if (!_embedder) {
    _embedder = await pipeline("feature-extraction", MODEL_NAME);
  }
  return _embedder;
}

export async function getCollection(): Promise<Collection> {
  if (!_collection) {
    _collection = await chroma.getOrCreateCollection({
      name: "cerebro_nodes",
      metadata: { "hnsw:space": "cosine" },
    });
  }
  return _collection;
}

export async function embedText(text: string): Promise<number[]> {
  const embedder = await getEmbedder();
  const output   = await embedder(text, { pooling: "mean", normalize: true });
  return Array.from(output.data as Float32Array);
}

export async function indexEntityVector(entity: {
  id:             string;
  label:          string;
  notes:          string | null;
  epistemic_mode: string;
  type:           string;
  fictional_world: string | null;
}): Promise<void> {
  const collection = await getCollection();
  const text       = [entity.label, entity.notes ?? ""].filter(Boolean).join(" ");
  const embedding  = await embedText(text);

  await collection.upsert({
    ids:        [entity.id],
    embeddings: [embedding],
    metadatas:  [{
      epistemic_mode:  entity.epistemic_mode,
      type:            entity.type,
      fictional_world: entity.fictional_world ?? "",
    }],
    documents: [text],
  });
}

export async function semanticSearch(
  query:          string,
  epistemicMode?: string,
  nResults        = 10,
) {
  const collection = await getCollection();
  const embedding  = await embedText(query);

  const where = epistemicMode
    ? { epistemic_mode: { "$eq": epistemicMode } }
    : undefined;

  return collection.query({
    queryEmbeddings: [embedding],
    nResults,
    where,
  });
}
```

---

### Step 3.4 — Quarantine SQLite store — `src/quarantine/schema.ts`

```typescript
import Database from "better-sqlite3";
import path from "path";

const QDB_PATH = process.env.QUARANTINE_DB ?? "/srv/cerebro/quarantine.db";

let _qdb: Database.Database | null = null;

export function getQuarantineDb(): Database.Database {
  if (!_qdb) {
    _qdb = new Database(QDB_PATH);
    _qdb.pragma("journal_mode = WAL");
    _qdb.pragma("foreign_keys = ON");
  }
  return _qdb;
}

export function bootstrapQuarantineSchema(): void {
  const db = getQuarantineDb();

  db.exec(`
    CREATE TABLE IF NOT EXISTS extraction_candidates (
      id                      TEXT PRIMARY KEY,
      subject_label           TEXT NOT NULL,
      subject_node_id         TEXT,
      predicate               TEXT NOT NULL,
      object_label            TEXT NOT NULL,
      object_node_id          TEXT,
      raw_sentence            TEXT NOT NULL,
      source_paper_uri        TEXT NOT NULL,
      source_section          TEXT,
      page_number             INTEGER,
      hedge_flag              INTEGER NOT NULL DEFAULT 0,
      hedge_text              TEXT,
      scope_qualifier         TEXT,
      negation_flag           INTEGER NOT NULL DEFAULT 0,
      suggested_confidence    TEXT NOT NULL,
      extractor_model         TEXT NOT NULL,
      extraction_method       TEXT NOT NULL DEFAULT 'llm',
      status                  TEXT NOT NULL DEFAULT 'pending',
      final_confidence        TEXT,
      final_subject_label     TEXT,
      final_predicate         TEXT,
      final_object_label      TEXT,
      epistemic_mode          TEXT NOT NULL DEFAULT 'empirical',
      fictional_world         TEXT,
      reviewer_notes          TEXT,
      source_reliability_tier TEXT,
      citation_type           TEXT NOT NULL DEFAULT 'direct',
      source_peer_reviewed    INTEGER,
      source_retracted        INTEGER NOT NULL DEFAULT 0,
      extracted_at            TEXT NOT NULL,
      reviewed_at             TEXT,
      promoted_edge_id        TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_status ON extraction_candidates(status);
    CREATE INDEX IF NOT EXISTS idx_source ON extraction_candidates(source_paper_uri);
    CREATE INDEX IF NOT EXISTS idx_hedge  ON extraction_candidates(hedge_flag);
  `);

  console.log("Quarantine SQLite schema bootstrapped.");
}
```

---

## Phase 4 — Graph Operations

### Step 4.1 — Entity CRUD — `src/graph/nodes.ts`

```typescript
import { v4 as uuid } from "uuid";
import { getConnection } from "../db/graph.js";
import { indexEntity }   from "../db/search.js";
import { indexEntityVector } from "../db/vectors.js";
import type { CerebroEntity, EpistemicMode } from "../types.js";

export async function createEntity(
  params: Omit<CerebroEntity, "id" | "created_at">
): Promise<CerebroEntity> {
  const conn   = getConnection();
  const entity: CerebroEntity = {
    id:             `node:${uuid()}`,
    created_at:     new Date().toISOString().slice(0, 10),
    ...params,
  };

  await conn.execute(
    `CREATE (:Entity {
      id: $id, label: $label, type: $type,
      epistemic_mode: $epistemic_mode,
      fictional_world: $fictional_world,
      aliases: $aliases,
      created_at: $created_at,
      notes: $notes
    })`,
    {
      id:             entity.id,
      label:          entity.label,
      type:           entity.type,
      epistemic_mode: entity.epistemic_mode,
      fictional_world: entity.fictional_world ?? "",
      aliases:        entity.aliases,
      created_at:     entity.created_at,
      notes:          entity.notes ?? "",
    }
  );

  // Mirror to Meilisearch for text search
  await indexEntity({
    id:             entity.id,
    label:          entity.label,
    aliases:        entity.aliases,
    type:           entity.type,
    epistemic_mode: entity.epistemic_mode,
    fictional_world: entity.fictional_world,
    notes:          entity.notes,
  });

  // Mirror to Chroma for semantic search
  await indexEntityVector({
    id:             entity.id,
    label:          entity.label,
    notes:          entity.notes,
    epistemic_mode: entity.epistemic_mode,
    type:           entity.type,
    fictional_world: entity.fictional_world,
  });

  return entity;
}

export async function getEntityById(id: string): Promise<CerebroEntity | null> {
  const conn   = getConnection();
  const result = await conn.execute(
    `MATCH (e:Entity {id: $id}) RETURN e.*`,
    { id }
  );
  const rows = result.getAll();
  if (!rows.length) return null;
  return rows[0]["e.*"] as CerebroEntity;
}

export async function findOrCreateEntity(
  label:         string,
  type:          string,
  epistemicMode: EpistemicMode = "empirical",
): Promise<CerebroEntity> {
  const conn   = getConnection();
  const result = await conn.execute(
    `MATCH (e:Entity) WHERE e.label = $label RETURN e.*`,
    { label }
  );
  const rows = result.getAll();
  if (rows.length) return rows[0]["e.*"] as CerebroEntity;

  return createEntity({
    label,
    type,
    aliases:        [],
    epistemic_mode: epistemicMode,
    fictional_world: null,
    notes:          null,
  });
}
```

---

### Step 4.2 — Assertion CRUD — `src/graph/edges.ts`

```typescript
import { v4 as uuid } from "uuid";
import { getConnection } from "../db/graph.js";
import type { CerebroAssertion } from "../types.js";

export async function createAssertion(
  params: Omit<CerebroAssertion, "id" | "created_at" | "updated_at">
): Promise<CerebroAssertion> {
  const conn      = getConnection();
  const now       = new Date().toISOString().slice(0, 10);
  const assertion: CerebroAssertion = {
    id:         `edge:${uuid()}`,
    created_at: now,
    updated_at: now,
    ...params,
  };

  // Enforce quarantine rule: inferred edges cannot self-promote
  if (assertion.source === "inferred" && assertion.confidence !== "speculative") {
    throw new Error(
      `Inferred edges must have confidence 'speculative'. ` +
      `Received: '${assertion.confidence}'. Use promote() to advance.`
    );
  }

  await conn.execute(
    `MATCH (a:Entity {id: $subject_id}), (b:Entity {id: $object_id})
     CREATE (a)-[:Assertion {
       id: $id,
       predicate: $predicate,
       epistemic_mode: $epistemic_mode,
       confidence: $confidence,
       fictional_world: $fictional_world,
       source: $source,
       evidence: $evidence,
       created_at: $created_at,
       updated_at: $updated_at
     }]->(b)`,
    {
      subject_id:     assertion.subject_id,
      object_id:      assertion.object_id,
      id:             assertion.id,
      predicate:      assertion.predicate,
      epistemic_mode: assertion.epistemic_mode,
      confidence:     assertion.confidence ?? "",
      fictional_world: assertion.fictional_world ?? "",
      source:         assertion.source,
      evidence:       assertion.evidence ?? "",
      created_at:     assertion.created_at,
      updated_at:     assertion.updated_at,
    }
  );

  return assertion;
}

export async function getAssertionsBySubject(subjectId: string) {
  const conn   = getConnection();
  const result = await conn.execute(
    `MATCH (a:Entity {id: $id})-[r:Assertion]->(b:Entity)
     RETURN r.*, b.id AS object_id, b.label AS object_label`,
    { id: subjectId }
  );
  return result.getAll();
}
```

---

### Step 4.3 — Junction and path queries — `src/graph/queries.ts`

```typescript
import { getConnection } from "../db/graph.js";

/** Nodes touched by both empirical and fictional edges */
export async function findJunctionNodes() {
  const conn   = getConnection();
  const result = await conn.execute(`
    MATCH (a:Entity)-[r:Assertion]->(b:Entity)
    WITH a, collect(DISTINCT r.epistemic_mode) AS modes
    WHERE 'empirical' IN modes AND 'fictional' IN modes
    RETURN a.label AS label, a.id AS id, modes
  `);
  return result.getAll();
}

/** All entities reachable from a starting node within N hops */
export async function shortestPaths(
  startId: string,
  endId:   string,
  maxHops  = 4,
) {
  const conn   = getConnection();
  const result = await conn.execute(
    `MATCH p = (a:Entity {id: $startId})-[:Assertion*1..${maxHops}]->(b:Entity {id: $endId})
     RETURN p`,
    { startId, endId }
  );
  return result.getAll();
}

/** Established claims with fewer than 2 live sources — integrity check */
export async function findUndercitedEstablished() {
  const conn   = getConnection();
  const result = await conn.execute(`
    MATCH (a:Entity)-[r:Assertion]->(b:Entity)
    WHERE r.confidence = 'established'
    WITH r.id AS edge_id, r.predicate AS predicate, count {
      MATCH ()-[c:CitedBy {assertion_id: r.id}]->(:Source {retracted: false})
    } AS live_sources
    WHERE live_sources < 2
    RETURN edge_id, predicate, live_sources
    ORDER BY live_sources ASC
  `);
  return result.getAll();
}

/** Edges whose only supporting sources are retracted */
export async function findOrphanedAfterRetraction() {
  const conn   = getConnection();
  const result = await conn.execute(`
    MATCH (a:Entity)-[r:Assertion]->(b:Entity)
    WHERE r.confidence IN ['established', 'probable']
    WITH r, count {
      MATCH ()-[c:CitedBy {assertion_id: r.id}]->(:Source {retracted: false})
    } AS live_sources
    WHERE live_sources = 0
    RETURN r.id AS edge_id, r.predicate, r.confidence
  `);
  return result.getAll();
}
```

---

## Phase 5 — Quarantine Pipeline

### Step 5.1 — Write candidates — `src/quarantine/ingest.ts`

```typescript
import { v4 as uuid } from "uuid";
import { getQuarantineDb } from "./schema.js";
import type { ExtractionCandidate, Confidence } from "../types.js";

export interface RawCandidate {
  subject_label:       string;
  predicate:           string;
  object_label:        string;
  raw_sentence:        string;
  source_paper_uri:    string;
  source_section?:     string;
  hedge_flag:          boolean;
  hedge_text?:         string;
  scope_qualifier?:    string;
  negation_flag:       boolean;
  suggested_confidence: Confidence;
  extractor_model:     string;
}

/** Pre-populate confidence tier based on extraction flags */
function deriveConfidence(raw: RawCandidate): Confidence {
  if (raw.negation_flag)                         return null; // force manual
  if (raw.hedge_flag && raw.scope_qualifier)     return "speculative";
  if (raw.hedge_flag)                            return "plausible";
  if (raw.source_section === "abstract")         return "probable";
  if (raw.source_section === "discussion")       return "plausible";
  return raw.suggested_confidence;
}

export function writeCandidate(raw: RawCandidate): ExtractionCandidate {
  const db        = getQuarantineDb();
  const candidate: ExtractionCandidate = {
    id:                    `candidate:${uuid()}`,
    subject_label:         raw.subject_label,
    subject_node_id:       null,
    predicate:             raw.predicate,
    object_label:          raw.object_label,
    object_node_id:        null,
    raw_sentence:          raw.raw_sentence,
    source_paper_uri:      raw.source_paper_uri,
    source_section:        raw.source_section ?? null,
    page_number:           null,
    hedge_flag:            raw.hedge_flag,
    hedge_text:            raw.hedge_text ?? null,
    scope_qualifier:       raw.scope_qualifier ?? null,
    negation_flag:         raw.negation_flag,
    suggested_confidence:  deriveConfidence(raw),
    extractor_model:       raw.extractor_model,
    extraction_method:     "llm",
    status:                "pending",
    final_confidence:      null,
    final_subject_label:   null,
    final_predicate:       null,
    final_object_label:    null,
    epistemic_mode:        "empirical",
    fictional_world:       null,
    reviewer_notes:        null,
    source_reliability_tier: null,
    citation_type:         "direct",
    source_peer_reviewed:  null,
    source_retracted:      false,
    extracted_at:          new Date().toISOString(),
    reviewed_at:           null,
    promoted_edge_id:      null,
  };

  db.prepare(`
    INSERT OR IGNORE INTO extraction_candidates
    (id, subject_label, predicate, object_label, raw_sentence,
     source_paper_uri, source_section, hedge_flag, hedge_text,
     scope_qualifier, negation_flag, suggested_confidence,
     extractor_model, extraction_method, status, epistemic_mode,
     citation_type, extracted_at)
    VALUES
    ($id, $subject_label, $predicate, $object_label, $raw_sentence,
     $source_paper_uri, $source_section, $hedge_flag, $hedge_text,
     $scope_qualifier, $negation_flag, $suggested_confidence,
     $extractor_model, $extraction_method, $status, $epistemic_mode,
     $citation_type, $extracted_at)
  `).run({
    ...candidate,
    hedge_flag:    candidate.hedge_flag    ? 1 : 0,
    negation_flag: candidate.negation_flag ? 1 : 0,
  });

  return candidate;
}

export function getPendingCandidates(): ExtractionCandidate[] {
  return getQuarantineDb()
    .prepare(`SELECT * FROM extraction_candidates WHERE status = 'pending'
              ORDER BY hedge_flag DESC, extracted_at ASC`)
    .all() as ExtractionCandidate[];
}
```

---

### Step 5.2 — Promote approved candidates — `src/quarantine/promote.ts`

```typescript
import { getQuarantineDb }  from "./schema.js";
import { findOrCreateEntity } from "../graph/nodes.js";
import { createAssertion }    from "../graph/edges.js";
import type { ExtractionCandidate } from "../types.js";

export async function promoteApproved(): Promise<void> {
  const db   = getQuarantineDb();
  const rows = db.prepare(`
    SELECT * FROM extraction_candidates
    WHERE status IN ('approved', 'edited')
      AND promoted_edge_id IS NULL
  `).all() as ExtractionCandidate[];

  console.log(`Promoting ${rows.length} approved candidates.`);

  for (const row of rows) {
    const subjectLabel = row.final_subject_label ?? row.subject_label;
    const predicate    = row.final_predicate     ?? row.predicate;
    const objectLabel  = row.final_object_label  ?? row.object_label;
    const confidence   = row.final_confidence    ?? row.suggested_confidence;

    const subjectEntity = await findOrCreateEntity(subjectLabel, "Unknown", row.epistemic_mode);
    const objectEntity  = await findOrCreateEntity(objectLabel,  "Unknown", row.epistemic_mode);

    const assertion = await createAssertion({
      subject_id:     subjectEntity.id,
      predicate,
      object_id:      objectEntity.id,
      epistemic_mode: row.epistemic_mode,
      confidence,
      fictional_world: row.fictional_world,
      source:         "citation",
      evidence:       `${row.source_paper_uri} — ${row.raw_sentence.slice(0, 120)}`,
    });

    db.prepare(`
      UPDATE extraction_candidates
      SET promoted_edge_id = $edge_id, status = 'promoted'
      WHERE id = $id
    `).run({ edge_id: assertion.id, id: row.id });

    console.log(`  Promoted: (${subjectLabel}) —[${predicate}]→ (${objectLabel}) [${confidence}]`);
  }

  console.log("Promotion complete.");
}
```

---

## Phase 6 — API Server

### Step 6.1 — Fastify entry point — `src/api/server.ts`

```typescript
import Fastify from "fastify";
import cors    from "@fastify/cors";
import { entityRoutes }     from "./routes/entities.js";
import { assertionRoutes }  from "./routes/assertions.js";
import { searchRoutes }     from "./routes/search.js";
import { quarantineRoutes } from "./routes/quarantine.js";

const app = Fastify({ logger: true });

await app.register(cors, { origin: true });
await app.register(entityRoutes,     { prefix: "/entities" });
await app.register(assertionRoutes,  { prefix: "/assertions" });
await app.register(searchRoutes,     { prefix: "/search" });
await app.register(quarantineRoutes, { prefix: "/quarantine" });

app.get("/health", async () => ({ status: "ok" }));

const PORT = Number(process.env.PORT ?? 3000);
await app.listen({ port: PORT, host: "0.0.0.0" });
console.log(`Cerebro API listening on :${PORT}`);
```

### Step 6.2 — Search routes — `src/api/routes/search.ts`

```typescript
import type { FastifyPluginAsync } from "fastify";
import { searchEntities }    from "../../db/search.js";
import { semanticSearch }    from "../../db/vectors.js";
import { findJunctionNodes } from "../../graph/queries.js";

export const searchRoutes: FastifyPluginAsync = async (app) => {

  // Full-text entity search
  app.get<{ Querystring: { q: string; mode?: string; world?: string } }>(
    "/entities",
    async (req) => {
      const { q, mode, world } = req.query;
      return searchEntities(q, mode, world);
    }
  );

  // Semantic similarity search
  app.get<{ Querystring: { q: string; mode?: string; n?: string } }>(
    "/semantic",
    async (req) => {
      const { q, mode, n } = req.query;
      return semanticSearch(q, mode, n ? parseInt(n) : 10);
    }
  );

  // Junction nodes — where empirical and fictional planes intersect
  app.get("/junctions", async () => findJunctionNodes());
};
```

---

## Phase 7 — Bootstrap Script and Docker

### Step 7.1 — `scripts/bootstrap.ts`

```typescript
import { bootstrapSchema }           from "../src/db/graph.js";
import { bootstrapSearchIndex }      from "../src/db/search.js";
import { bootstrapQuarantineSchema } from "../src/quarantine/schema.js";

console.log("Bootstrapping Cerebro...");
await bootstrapSchema();
await bootstrapSearchIndex();
bootstrapQuarantineSchema();
console.log("Bootstrap complete.");
```

### Step 7.2 — `docker-compose.yml`

```yaml
services:
  meilisearch:
    image: getmeili/meilisearch:latest
    ports:
      - "7700:7700"
    volumes:
      - ./meili_data:/meili_data
    environment:
      MEILI_MASTER_KEY: "${MEILI_KEY}"
    restart: unless-stopped

  chroma:
    image: chromadb/chroma:latest
    ports:
      - "8000:8000"
    volumes:
      - ./chroma_data:/chroma/chroma
    restart: unless-stopped
```

### Step 7.3 — `.env`

```bash
KUZU_PATH=/srv/cerebro/cerebro.db
QUARANTINE_DB=/srv/cerebro/quarantine.db
MEILI_HOST=http://localhost:7700
MEILI_KEY=your-master-key-here
CHROMA_PATH=http://localhost:8000
PORT=3000
```

---

## Phase 8 — Systemd Service

Run the API as a managed service on `ubuntu-server1`:

```ini
# /etc/systemd/system/cerebro.service

[Unit]
Description=Cerebro Knowledge Graph API
After=network.target docker.service
Requires=docker.service

[Service]
Type=simple
User=lcasinelli
WorkingDirectory=/srv/cerebro
EnvironmentFile=/srv/cerebro/.env
ExecStart=/usr/bin/node dist/api/server.js
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable cerebro
sudo systemctl start cerebro
sudo journalctl -u cerebro -f   # tail logs
```

---

## Phase 9 — Implementation Order

Execute in this sequence. Each phase is independently testable
before the next begins.

| Phase | Deliverable                              | Test                                      |
|-------|------------------------------------------|-------------------------------------------|
| 1     | Project scaffold, packages installed     | `npm run bootstrap` completes             |
| 2     | `src/types.ts`                           | TypeScript compiles with no errors        |
| 3     | `src/db/graph.ts` — Kùzu schema          | `bootstrapSchema()` creates tables        |
| 3     | `src/db/search.ts` — Meilisearch config  | Index created, settings accepted          |
| 3     | `src/db/vectors.ts` — Chroma + embedding | `embedText("test")` returns float array   |
| 3     | `src/quarantine/schema.ts` — SQLite      | `bootstrapQuarantineSchema()` creates table|
| 4     | `src/graph/nodes.ts`                     | `createEntity()` writes and mirrors       |
| 4     | `src/graph/edges.ts`                     | `createAssertion()` rejects inferred      |
| 4     | `src/graph/queries.ts`                   | Junction query returns expected nodes     |
| 5     | `src/quarantine/ingest.ts`               | `writeCandidate()` writes to SQLite       |
| 5     | `src/quarantine/promote.ts`              | `promoteApproved()` creates graph edges   |
| 6     | `src/api/server.ts` + routes             | `GET /health` returns `{ status: "ok" }`  |
| 7     | `docker-compose.yml`, `.env`             | Meilisearch and Chroma containers healthy |
| 8     | `cerebro.service`                        | `systemctl status cerebro` shows active   |

---

## Appendix — Key Invariants

These must never be violated by any code path:

1. **Inferred edges cannot self-promote.** `createAssertion()` throws if
   `source === "inferred"` and `confidence !== "speculative"`.

2. **Nothing bypasses quarantine.** Extracted candidates are written to
   SQLite first. `promoteApproved()` is run explicitly, never automatically.

3. **All three stores stay in sync.** `createEntity()` writes to Kùzu,
   Meilisearch, and Chroma in the same call. If any write fails, the
   entity is in a partial state — implement compensating rollback if
   this becomes a reliability concern at scale.

4. **Fictional edges carry `confidence: null`.** `createAssertion()` should
   validate this and reject fictional edges with a non-null confidence.

5. **Established claims require 2 independent sources.** Validated by
   `findUndercitedEstablished()` — run this as a periodic integrity check,
   not a write-time guard (source nodes are added after the edge).

---

*Document version: 2026-03-18 | System: Cerebro*
*Companion documents: cerebro-kg-design.md · extraction-design-schema.md · citation-inclusion-design-schema.md*
