# Cerebro — Extraction Pipeline Design & Schema

> Why automatic knowledge extraction from research literature is unreliable by default,
> how to use it responsibly, and the full schema for a human-gated ingestion pipeline.

---

## 1. Why Full Automation Is the Wrong Architecture

Automated triple extraction from research papers is a solved problem in the
same way that machine translation is a solved problem: it works well enough
to be useful, and badly enough to be dangerous if you trust it uncritically.

The fundamental issue is that a research paper is not a list of facts.
It is an argument — a rhetorical structure where conclusions are hedged,
scoped, qualified, and entangled with the methodology that produced them.
When you read:

> "Our results suggest a moderate correlation between X and Y
> in populations exhibiting Z under controlled conditions."

You parse that as a heavily conditioned, domain-specific, probabilistic claim
with significant scope qualifiers. An extraction pipeline sees a sentence
and must answer: is this a fact? Who are the entities? What is the relationship?
It systematically flattens every qualification in that sentence.

Feeding extracted triples directly into Cerebro without human review would
corrupt the epistemic integrity of the graph in ways that are nearly impossible
to audit after the fact. The damage is silent — the graph looks populated and
coherent while containing materially false or distorted claims.

### 1.1 Failure Mode Catalogue

#### Hedging Blindness

The most insidious failure. Extractors are trained to find assertions.
Hedged claims — "may influence," "is consistent with," "does not rule out,"
"preliminary evidence suggests" — get promoted to confident edges because
the syntactic pattern superficially resembles a declarative assertion.

The extracted triple looks correct. The original claim was not.

```
Source:    "Drug X may reduce inflammation in some patients."
Extracted: (Drug X) —[reduces]→ (inflammation)   ← hedge erased
```

#### Scope Collapse

Qualifying conditions — population, experimental context, measurement method —
are routinely dropped, leaving a triple that implies a stronger and more general
claim than the paper makes.

```
Source:    "In mice, drug X reduces inflammation by 40%."
Extracted: (Drug X) —[reduces]→ (inflammation, 40%)   ← "in mice" gone
```

#### Coreference Collapse

"Einstein developed this theory. He later revised it."
The extractor must resolve "He" to Einstein and "this theory" to special relativity
across paragraph boundaries. Errors here produce phantom entities or
edges attributed to the wrong subject — and the error is invisible in the triple.

#### Negation Failure

"X does not cause Y" is routinely extracted as a positive X→Y edge by
weaker models. Even strong models fail on double negation, scoped negation,
and negation buried in subordinate clauses. This is the failure mode with
the highest potential to actively mislead: a refuted claim entered as a fact.

```
Source:    "We found no evidence that X causes Y."
Extracted: (X) —[causes]→ (Y)   ← sign of the claim inverted
```

#### Implicit Conclusion Blindness

Many of a paper's most important conclusions are never stated as explicit
declarative sentences. They live in the discussion section as implications,
in the structure of a figure, in what the authors chose not to address,
or in the gap between what was hypothesized and what was found.
No extractor reaches these.

#### Temporal and Modal Conflation

"Scientists believed X until 1987" and "X is true" are extracted identically
by pipelines that do not model tense and modality. Historical claims,
retracted findings, and counterfactuals all collapse into the same
present-tense positive assertion.

### 1.2 Where Extraction Is Reliable

Not all content is equally hazardous. Certain paper structures yield
meaningfully cleaner extractions and are worth targeting specifically:

| Content type                    | Reliability | Notes                                      |
|---------------------------------|-------------|---------------------------------------------|
| Structured abstract — Results   | High        | Written to be declarative, scoped sentences |
| Structured abstract — Conclusions | High      | Same — minimal hedging by convention        |
| Numerical findings              | High        | Syntactically unambiguous quantities        |
| Named entity recognition        | High        | Well-solved for biomedical/scientific NER   |
| Citation graphs                 | High        | "Paper A cites B in context of C"           |
| Discussion section conclusions  | Medium      | Heavy hedging, high value — review carefully|
| Methods section                 | Low         | Procedural, rarely produces useful triples  |
| Introduction / background       | Low         | Reporting others' work, attribution complex |

**The pattern:** extraction is reliable where the text is already behaving
like a triple. It degrades proportionally to how much interpretive work
is being done.

### 1.3 The Irreducible Epistemological Argument

Even with a hypothetically perfect extractor, human approval would still be
mandatory — because the question of whether a paper's conclusion belongs in
*your* knowledge graph is not purely a question of extraction accuracy.

It is a question of whether you *endorse* that conclusion, how much weight
you assign it relative to competing literature, and how it fits the epistemic
framework Cerebro is built to represent. That judgment is irreducibly yours.
The extractor surfaces candidates. It cannot do epistemology.

The quarantine wall described in this document is not a temporary scaffold
to be removed when models improve. It is a permanent architectural feature.

---

## 2. Recommended Extraction Tools

### 2.1 LLM-Based Structured Extraction (Primary)

**Best current approach** for Cerebro's use case. A capable local model
prompted with explicit structure requirements outperforms classical IE pipelines
on hedging recognition, scope preservation, and negation handling.

**Recommended local models (via Ollama):**
- `llama3:70b` — best quality, requires ~40GB VRAM or CPU offload
- `mistral:7b` — strong quality/resource trade-off, 8GB RAM
- `phi3:medium` — surprisingly capable at structured extraction, 8GB RAM

**Extraction prompt template:**

```
You are a scientific knowledge extraction system.
Extract all factual claims from the following text as a JSON array of triples.

For each triple, provide:
- subject: the entity making or being described
- predicate: the relationship or property
- object: the entity or value
- raw_sentence: the exact sentence the triple was extracted from
- hedge_flag: true if the claim is hedged, conditional, or uncertain
- hedge_text: the specific hedging language used (or null)
- scope_qualifier: any population, condition, or scope restriction (or null)
- negation_flag: true if this is a negative claim
- suggested_confidence: one of [axiomatic, established, probable, plausible, speculative]

Be conservative. If a claim is ambiguous, mark hedge_flag true.
If you cannot extract a clean triple, do not extract it.
Return ONLY valid JSON. No preamble.

TEXT:
{paper_section}
```

### 2.2 spaCy + scispaCy (NER Layer)

Use as a pre-processing step to identify and normalize named entities
*before* passing text to the LLM extractor. scispaCy models are trained
on biomedical literature and handle entity aliasing (aspirin /
acetylsalicylic acid / ASA) far better than general models.

```bash
pip install spacy scispacy
pip install https://s3-us-west-2.amazonaws.com/ai2-s3-scispacy/releases/v0.5.3/en_core_sci_lg-0.5.3.tar.gz
```

### 2.3 Rebel (Secondary / Comparison)

Babelscape's end-to-end relation extraction model. Useful as a second opinion
alongside LLM extraction — where both agree, confidence in the candidate
triple is higher. Where they disagree, flag for careful human review.

```bash
pip install transformers
# Model: Babelscape/rebel-large
```

### 2.4 What Not to Use

**OpenIE (Stanford, AllenNLP)** — high recall, very low precision. Produces
enormous triple volumes, most of which are noise. Not appropriate as a
primary extraction method for a curated personal knowledge graph.

---

## 3. Pipeline Architecture

```
Paper PDF / URL
      │
      ▼
┌─────────────────────────────┐
│  Pre-processing             │
│  · PDF → text (pypdf2)      │
│  · Section segmentation     │
│  · NER normalization (spaCy)│
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│  Extraction                 │
│  · LLM structured prompt    │
│  · Rebel (optional, compare)│
│  · Output: candidate JSON   │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│  Quarantine store           │  ← candidates live here, never in main graph
│  (SQLite or Postgres table) │
│  status: pending            │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│  Human review UI / CLI      │
│  · View candidate + source  │
│  · Approve / Edit / Reject  │
│  · Set final confidence      │
└──────────────┬──────────────┘
               │
    ┌──────────┴──────────┐
    ▼                     ▼
Approved               Rejected
    │                     │
    ▼                     ▼
Main graph           Discard log
(Kùzu)               (audit trail)
source: "citation"
confidence: human-set
```

---

## 4. Quarantine Store Schema

All extracted candidates are written here first. Nothing bypasses this table.

### 4.1 SQL Schema

```sql
CREATE TABLE extraction_candidates (
  id                  TEXT PRIMARY KEY,         -- candidate:uuid
  subject_label       TEXT NOT NULL,
  subject_node_id     TEXT,                     -- resolved UUID, null until entity-linked
  predicate           TEXT NOT NULL,
  object_label        TEXT NOT NULL,
  object_node_id      TEXT,                     -- resolved UUID, null until entity-linked

  raw_sentence        TEXT NOT NULL,            -- exact source sentence
  source_paper_uri    TEXT NOT NULL,            -- DOI, arXiv ID, or local path
  source_section      TEXT,                     -- abstract | results | discussion | etc.
  page_number         INTEGER,

  hedge_flag          BOOLEAN NOT NULL DEFAULT FALSE,
  hedge_text          TEXT,                     -- "may", "suggests", "consistent with"
  scope_qualifier     TEXT,                     -- "in mice", "in cohort aged 40-60"
  negation_flag       BOOLEAN NOT NULL DEFAULT FALSE,

  suggested_confidence TEXT NOT NULL,           -- extractor's suggestion
  extractor_model     TEXT NOT NULL,            -- which model/version extracted this
  extraction_method   TEXT NOT NULL,            -- "llm" | "rebel" | "manual"

  status              TEXT NOT NULL DEFAULT 'pending',
                                                -- pending | approved | rejected | edited
  final_confidence    TEXT,                     -- human-set on approval
  final_subject_label TEXT,                     -- human-corrected if edited
  final_predicate     TEXT,                     -- human-corrected if edited
  final_object_label  TEXT,                     -- human-corrected if edited
  epistemic_mode      TEXT DEFAULT 'empirical', -- human assigns on approval
  fictional_world     TEXT,                     -- only if epistemic_mode = fictional

  reviewer_notes      TEXT,
  extracted_at        TEXT NOT NULL,
  reviewed_at         TEXT,
  promoted_edge_id    TEXT                      -- FK to Kùzu edge UUID after promotion
);

CREATE INDEX idx_status ON extraction_candidates(status);
CREATE INDEX idx_source ON extraction_candidates(source_paper_uri);
CREATE INDEX idx_hedge  ON extraction_candidates(hedge_flag);
```

### 4.2 Python Dataclass Equivalent

```python
from dataclasses import dataclass, field
from typing import Optional
from datetime import datetime
import uuid

@dataclass
class ExtractionCandidate:
    # Identity
    id:                  str = field(default_factory=lambda: f"candidate:{uuid.uuid4()}")

    # Triple content (raw from extractor)
    subject_label:       str = ""
    subject_node_id:     Optional[str] = None
    predicate:           str = ""
    object_label:        str = ""
    object_node_id:      Optional[str] = None

    # Provenance
    raw_sentence:        str = ""
    source_paper_uri:    str = ""
    source_section:      Optional[str] = None
    page_number:         Optional[int] = None

    # Epistemic flags from extractor
    hedge_flag:          bool = False
    hedge_text:          Optional[str] = None
    scope_qualifier:     Optional[str] = None
    negation_flag:       bool = False
    suggested_confidence: str = "speculative"
    extractor_model:     str = ""
    extraction_method:   str = "llm"

    # Review fields (human-populated)
    status:              str = "pending"
    final_confidence:    Optional[str] = None
    final_subject_label: Optional[str] = None
    final_predicate:     Optional[str] = None
    final_object_label:  Optional[str] = None
    epistemic_mode:      str = "empirical"
    fictional_world:     Optional[str] = None
    reviewer_notes:      Optional[str] = None

    # Timestamps
    extracted_at:        str = field(default_factory=lambda: datetime.utcnow().isoformat())
    reviewed_at:         Optional[str] = None
    promoted_edge_id:    Optional[str] = None
```

---

## 5. Extraction Script Skeleton

```python
import ollama
import json
import sqlite3
import uuid
from datetime import datetime
from pathlib import Path

EXTRACTION_PROMPT = """
You are a scientific knowledge extraction system.
Extract all factual claims from the following text as a JSON array of triples.

For each triple, provide:
- subject: entity label
- predicate: relationship
- object: entity or value
- raw_sentence: exact source sentence
- hedge_flag: true if hedged, conditional, or uncertain
- hedge_text: specific hedging language or null
- scope_qualifier: population or condition restriction or null
- negation_flag: true if this is a negative claim
- suggested_confidence: one of [axiomatic, established, probable, plausible, speculative]

Be conservative. Ambiguous claims get hedge_flag: true.
If a clean triple cannot be extracted, omit it.
Return ONLY valid JSON. No preamble or explanation.

TEXT:
{text}
"""

def extract_from_section(
    text: str,
    source_paper_uri: str,
    source_section: str,
    model: str = "mistral"
) -> list[dict]:
    prompt = EXTRACTION_PROMPT.format(text=text)
    response = ollama.chat(
        model=model,
        messages=[{"role": "user", "content": prompt}]
    )
    raw = response["message"]["content"].strip()

    # Strip markdown fences if model includes them
    if raw.startswith("```"):
        raw = raw.split("```")[1]
        if raw.startswith("json"):
            raw = raw[4:]
    raw = raw.strip()

    try:
        triples = json.loads(raw)
    except json.JSONDecodeError:
        print(f"JSON parse failure — review raw output:\n{raw}")
        return []

    candidates = []
    for t in triples:
        candidates.append({
            "id":                 f"candidate:{uuid.uuid4()}",
            "subject_label":      t.get("subject", ""),
            "predicate":          t.get("predicate", ""),
            "object_label":       t.get("object", ""),
            "raw_sentence":       t.get("raw_sentence", ""),
            "source_paper_uri":   source_paper_uri,
            "source_section":     source_section,
            "hedge_flag":         t.get("hedge_flag", False),
            "hedge_text":         t.get("hedge_text"),
            "scope_qualifier":    t.get("scope_qualifier"),
            "negation_flag":      t.get("negation_flag", False),
            "suggested_confidence": t.get("suggested_confidence", "speculative"),
            "extractor_model":    model,
            "extraction_method":  "llm",
            "status":             "pending",
            "extracted_at":       datetime.utcnow().isoformat(),
        })
    return candidates


def write_to_quarantine(candidates: list[dict], db_path: str = "./cerebro_quarantine.db"):
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    for c in candidates:
        cur.execute("""
            INSERT OR IGNORE INTO extraction_candidates
            (id, subject_label, predicate, object_label, raw_sentence,
             source_paper_uri, source_section, hedge_flag, hedge_text,
             scope_qualifier, negation_flag, suggested_confidence,
             extractor_model, extraction_method, status, extracted_at)
            VALUES
            (:id, :subject_label, :predicate, :object_label, :raw_sentence,
             :source_paper_uri, :source_section, :hedge_flag, :hedge_text,
             :scope_qualifier, :negation_flag, :suggested_confidence,
             :extractor_model, :extraction_method, :status, :extracted_at)
        """, c)
    conn.commit()
    conn.close()
    print(f"Wrote {len(candidates)} candidates to quarantine.")
```

---

## 6. Human Review CLI

A minimal approval interface for the terminal. A web UI can be built on top
of the same SQLite queries when the volume warrants it.

```python
import sqlite3
from datetime import datetime

CONFIDENCE_LEVELS = ["axiomatic", "established", "probable", "plausible", "speculative"]

def review_pending(db_path: str = "./cerebro_quarantine.db"):
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    rows = cur.execute("""
        SELECT * FROM extraction_candidates
        WHERE status = 'pending'
        ORDER BY hedge_flag DESC, extracted_at ASC
    """).fetchall()

    if not rows:
        print("No pending candidates.")
        return

    print(f"\n{len(rows)} candidates pending review.\n")

    for row in rows:
        print("─" * 60)
        print(f"  ID:         {row['id']}")
        print(f"  Triple:     ({row['subject_label']}) —[{row['predicate']}]→ ({row['object_label']})")
        print(f"  Source:     {row['source_section']} — {row['source_paper_uri']}")
        print(f"  Sentence:   {row['raw_sentence']}")
        print(f"  Hedged:     {bool(row['hedge_flag'])}  {row['hedge_text'] or ''}")
        print(f"  Scope:      {row['scope_qualifier'] or 'none'}")
        print(f"  Negation:   {bool(row['negation_flag'])}")
        print(f"  Suggested:  {row['suggested_confidence']}")
        print()

        action = input("  [a]pprove / [e]dit / [r]eject / [s]kip: ").strip().lower()

        if action == "r":
            note = input("  Rejection reason (optional): ").strip()
            cur.execute("""
                UPDATE extraction_candidates
                SET status='rejected', reviewer_notes=?, reviewed_at=?
                WHERE id=?
            """, (note or None, datetime.utcnow().isoformat(), row["id"]))
            conn.commit()
            print("  → Rejected.")

        elif action == "a":
            print(f"  Confidence levels: {', '.join(CONFIDENCE_LEVELS)}")
            conf = input(f"  Confidence [{row['suggested_confidence']}]: ").strip()
            if conf not in CONFIDENCE_LEVELS:
                conf = row["suggested_confidence"]
            cur.execute("""
                UPDATE extraction_candidates
                SET status='approved', final_confidence=?, reviewed_at=?
                WHERE id=?
            """, (conf, datetime.utcnow().isoformat(), row["id"]))
            conn.commit()
            print(f"  → Approved ({conf}).")

        elif action == "e":
            subj = input(f"  Subject [{row['subject_label']}]: ").strip() or row["subject_label"]
            pred = input(f"  Predicate [{row['predicate']}]: ").strip() or row["predicate"]
            obj  = input(f"  Object [{row['object_label']}]: ").strip() or row["object_label"]
            print(f"  Confidence levels: {', '.join(CONFIDENCE_LEVELS)}")
            conf = input(f"  Confidence [{row['suggested_confidence']}]: ").strip()
            if conf not in CONFIDENCE_LEVELS:
                conf = row["suggested_confidence"]
            note = input("  Notes (optional): ").strip()
            cur.execute("""
                UPDATE extraction_candidates
                SET status='edited',
                    final_subject_label=?, final_predicate=?, final_object_label=?,
                    final_confidence=?, reviewer_notes=?, reviewed_at=?
                WHERE id=?
            """, (subj, pred, obj, conf, note or None,
                  datetime.utcnow().isoformat(), row["id"]))
            conn.commit()
            print(f"  → Edited and approved ({conf}).")

        else:
            print("  → Skipped.")

    conn.close()
```

---

## 7. Promotion to Main Graph

Approved and edited candidates are promoted to Kùzu via a separate
promotion step. This is deliberately not automatic — run it explicitly
after reviewing a batch.

```python
import kuzu
import sqlite3
import uuid
from datetime import datetime

def promote_approved(
    quarantine_db: str = "./cerebro_quarantine.db",
    kuzu_db_path:  str = "./cerebro.db"
):
    qconn = sqlite3.connect(quarantine_db)
    qconn.row_factory = sqlite3.Row
    qcur = qconn.cursor()

    db   = kuzu.Database(kuzu_db_path)
    kconn = kuzu.Connection(db)

    rows = qcur.execute("""
        SELECT * FROM extraction_candidates
        WHERE status IN ('approved', 'edited')
          AND promoted_edge_id IS NULL
    """).fetchall()

    print(f"Promoting {len(rows)} approved candidates to graph.")

    for row in rows:
        subj = row["final_subject_label"] or row["subject_label"]
        pred = row["final_predicate"]     or row["predicate"]
        obj  = row["final_object_label"]  or row["object_label"]
        conf = row["final_confidence"]    or row["suggested_confidence"]
        edge_id = f"edge:{uuid.uuid4()}"

        # Upsert subject and object nodes (by label — entity resolution
        # should be run before this step to assign proper node IDs)
        for label in [subj, obj]:
            kconn.execute("""
                MERGE (e:Entity {label: $label})
                ON CREATE SET e.id = $id, e.created_at = $ts
            """, {"label": label, "id": f"node:{uuid.uuid4()}", "ts": datetime.utcnow().isoformat()})

        # Create edge
        kconn.execute("""
            MATCH (a:Entity {label: $subj}), (b:Entity {label: $obj})
            CREATE (a)-[:Assertion {
                id:             $edge_id,
                predicate:      $pred,
                epistemic_mode: $emode,
                confidence:     $conf,
                source:         'citation',
                evidence:       $evidence,
                created_at:     $ts,
                updated_at:     $ts
            }]->(b)
        """, {
            "subj":     subj,
            "obj":      obj,
            "edge_id":  edge_id,
            "pred":     pred,
            "emode":    row["epistemic_mode"] or "empirical",
            "conf":     conf,
            "evidence": f"{row['source_paper_uri']} — {row['raw_sentence'][:120]}",
            "ts":       datetime.utcnow().isoformat()
        })

        # Mark promoted
        qcur.execute("""
            UPDATE extraction_candidates
            SET promoted_edge_id = ?, status = 'promoted'
            WHERE id = ?
        """, (edge_id, row["id"]))
        qconn.commit()
        print(f"  Promoted: ({subj}) —[{pred}]→ ({obj})  [{conf}]")

    qconn.close()
    print("Promotion complete.")
```

---

## 8. Confidence Pre-population Rules

The extractor's suggested confidence is a starting hint, not an inherited value.
These rules govern how the review CLI pre-populates the confidence field:

| Extractor flags                          | Pre-populated confidence |
|------------------------------------------|--------------------------|
| `hedge_flag: true` + `scope_qualifier`   | `speculative`            |
| `hedge_flag: true`, no scope             | `plausible`              |
| `negation_flag: true`                    | Manual review — no pre-pop |
| `hedge_flag: false`, structured abstract | `probable`               |
| `hedge_flag: false`, discussion section  | `plausible`              |
| Numerical finding, results section       | `probable`               |
| Definition or axiomatic claim            | `established`            |

The human always overrides. These are defaults to reduce keystrokes, not assignments.

---

## 9. Integration with Cerebro Edge Schema

Promoted edges use the full Cerebro edge schema from `cerebro-kg-design.md`.
The mapping from quarantine fields to edge fields:

| Quarantine field           | Cerebro edge field    | Notes                              |
|----------------------------|-----------------------|------------------------------------|
| `final_subject_label`      | `subject`             | Resolved to node UUID              |
| `final_predicate`          | `predicate`           |                                    |
| `final_object_label`       | `object`              | Resolved to node UUID              |
| `epistemic_mode`           | `epistemic_mode`      | Human-set during review            |
| `final_confidence`         | `confidence`          | Human-set, extractor hint only     |
| hardcoded: `"citation"`    | `source`              | All extracted edges are citations  |
| `source_paper_uri` + sentence | `evidence`         | URI + truncated raw sentence       |
| `fictional_world`          | `fictional_world`     | Null for all empirical extractions |

---

## 10. Recommended Workflow Per Paper

1. Run PDF through section segmenter — extract Abstract, Results, Discussion separately.
2. Run NER normalization (scispaCy) to standardize entity labels.
3. Run LLM extraction on Results and Conclusions subsections first —
   highest reliability, lowest hedge rate.
4. Run LLM extraction on Discussion section second — flag all candidates
   from this section with elevated scrutiny regardless of hedge_flag value.
5. Skip Methods and Introduction sections unless hunting for specific procedural facts.
6. Review quarantine queue — sort hedge_flag: true candidates to end of queue,
   negation_flag: true candidates require manual confidence assignment.
7. Run promotion step explicitly after completing a review session.
8. Run entity resolution pass to collapse duplicate nodes
   (same entity entered under different labels).

---

*Document version: 2026-03-18 | System: Cerebro | Companion: cerebro-kg-design.md*
