# Cerebro — Citation & Source Storage Design

> Citations for high-confidence claims are not metadata.
> They are load-bearing structure. This document specifies how to store,
> relate, and validate them within the Cerebro knowledge graph.

---

## 1. What a Citation Must Do

Storing a citation as a string field on an edge — a DOI, a URL,
a bibliographic note — is sufficient until you need to ask:

- "What does Cerebro believe on the basis of this paper alone?"
- "This paper was retracted — which edges need review?"
- "Which `established` claims have only one source?"
- "Do I have any counter-evidence recorded for this assertion?"

A string field cannot answer any of these questions.
A citation that is only a string is a dead end — it proves origin but
cannot participate in graph traversal, integrity validation, or
epistemic auditing.

For `established` and `axiomatic` claims specifically, the citation is doing
heavier lifting than for lower tiers. An `established` claim is established
*because* of its sources. Strip the citation and you have stripped the
justification for the confidence rating itself. The citation is not decoration
hanging off the edge — it is the reason the edge has the tier it has.

### 1.1 Three Distinct Functions

| Function            | What it requires                                        |
|---------------------|---------------------------------------------------------|
| Provenance          | Where did this claim come from                          |
| Verifiability       | Can you or someone else go check it                     |
| Epistemic weight    | How does source quality affect the confidence rating    |

All three must be satisfiable from the stored data.
A URI alone satisfies only the first two.

---

## 2. Citations as First-Class Graph Nodes

Sources are nodes in the Cerebro graph, connected to assertion edges
via explicit citation relationships. This enables:

- **Retraction cascade queries** — find all edges whose only support
  is a retracted source
- **Source convergence modelling** — when three independent sources
  support the same edge, that convergence is itself representable and queryable
- **Counter-evidence records** — dissenting sources are stored as
  `refuting` citation edges, giving a complete epistemic picture
  rather than a confirmation stack

### 2.1 Graph Structure

```
(Subject Entity) ──[Assertion, confidence: established]──► (Object Entity)
                           │
                           └──[supported_by]──► (Source Node)
                                                      │
                                               source_type: primary_research
                                               reliability_tier: primary
                                               peer_reviewed: true
                                               retracted: false
```

A single assertion edge may have multiple `supported_by` relationships
pointing to different source nodes, including sources of `citation_type: refuting`.

---

## 3. Source Node Schema

Every distinct publication, document, or reference is a single node.
Two assertion edges citing the same paper point to the same source node.

### 3.1 Full Schema

```python
{
  "id":               "node:source:uuid",
  "type":             "Source",
  "label":            "Human-readable title or short reference",

  # Classification
  "source_type":      "primary_research",   # see taxonomy — Section 3.2
  "reliability_tier": "primary",            # primary | secondary | tertiary | grey

  # Identifiers
  "uri":              "https://...",         # canonical URL
  "doi":              "10.xxxx/...",         # null if not applicable
  "isbn":             null,                  # for books
  "arxiv_id":         null,
  "local_path":       null,                  # path on server if stored locally

  # Bibliographic
  "authors":          ["Author A", "Author B"],
  "publication_year": 2023,
  "publisher":        "Publisher Name",
  "journal":          null,                  # if applicable
  "volume":           null,
  "issue":            null,

  # Epistemic status
  "peer_reviewed":    true,
  "retracted":        false,
  "retraction_uri":   null,                  # URI of retraction notice if retracted
  "retraction_date":  null,

  # Housekeeping
  "accessed_at":      "2026-03-18",
  "added_at":         "2026-03-18",
  "notes":            null
}
```

### 3.2 Source Type Taxonomy

| Value               | Examples                                                    |
|---------------------|-------------------------------------------------------------|
| `primary_research`  | Journal article, thesis, original dataset                   |
| `systematic_review` | Meta-analysis, Cochrane review, systematic literature review|
| `encyclopedia`      | Britannica, Stanford Encyclopedia of Philosophy             |
| `reference_work`    | Handbook, textbook, dictionary                              |
| `official_record`   | Government document, legal filing, institutional record     |
| `contemporaneous`   | Newspaper of record, dated correspondence, diary            |
| `monograph`         | Scholarly book                                              |
| `grey`              | Preprint, working paper, blog post by domain expert         |

### 3.3 Reliability Tier

| Tier        | Criteria                                                             |
|-------------|----------------------------------------------------------------------|
| `primary`   | Original research, official records, first-hand accounts            |
| `secondary` | Synthesis or interpretation of primary sources                      |
| `tertiary`  | Synthesis of secondary sources — encyclopedias, textbooks           |
| `grey`      | Not formally published or peer reviewed — use with explicit caution |

---

## 4. Citation Relationship Schema

The relationship between an assertion edge and a source node carries
its own metadata. This is a separate relationship type in Kùzu.

### 4.1 Full Schema

```python
{
  "id":               "citation:uuid",
  "assertion_id":     "edge:uuid",        # the assertion edge being supported
  "source_id":        "node:source:uuid",

  # How the source relates to the claim
  "citation_type":    "direct",           # see taxonomy — Section 4.2
  "page_or_section":  "p. 47",            # null if not applicable
  "quote":            null,               # verbatim excerpt — short, for axiomatic claims

  "added_at":         "2026-03-18"
}
```

### 4.2 Citation Type Taxonomy

| Value        | Meaning                                                          |
|--------------|------------------------------------------------------------------|
| `direct`     | Source explicitly states the claim                              |
| `indirect`   | Source implies or entails the claim without stating it directly |
| `refuting`   | Counter-evidence — source argues against or contradicts claim   |
| `contextual` | Provides background without directly supporting the claim       |

The `refuting` type is the most commonly omitted. For `established` claims
in particular, storing known dissenting sources as `refuting` citation edges
is not optional — it is what distinguishes an honest epistemic record from
a confirmation stack.

---

## 5. Minimum Citation Requirements by Confidence Tier

The confidence rating must be *enforced* by citation requirements.
A tier unmoored from evidence constraints is a feeling, not an assessment.

| Confidence tier | Min sources required | Required source types                                   |
|-----------------|----------------------|---------------------------------------------------------|
| `axiomatic`     | 1                    | Any — record the canonical reference for the definition |
| `established`   | 2 independent        | At least 1 `primary_research` or `official_record`     |
| `probable`      | 1                    | Any `primary_research`, `systematic_review`, or `reference_work` |
| `plausible`     | 1                    | Any — including `grey`                                  |
| `speculative`   | 0                    | Citation optional                                       |

**"Independent"** means different authors, different institutions, different
methodologies. Two papers from the same lab do not constitute independent
sources. A paper and the textbook that cites it do not constitute independent
sources.

### 5.1 Integrity Violations

The following states are data integrity violations and should be flagged
by a validation pass:

- Edge rated `established` with fewer than 2 source nodes attached
- Edge rated `established` with no `primary` or `secondary` reliability tier sources
- Edge rated `established` with only `grey` reliability tier sources
- Edge rated `established` or `probable` where all attached sources
  have `retracted: true`
- Edge rated `axiomatic` with no source node attached

---

## 6. The Special Case of `axiomatic`

`Axiomatic` claims are not established by the weight of accumulated evidence.
They are established by authoritative definition or uncontested historical record.
The citation serves a different function than for `established` claims.

For `established`: the citation stack *justifies* the confidence rating
by demonstrating evidential convergence.

For `axiomatic`: the citation is a **canonical reference** — the single most
authoritative source for the definition or settled fact. You record it not
because doubt requires rebuttal, but because the source of record should
always be traceable.

Appropriate sources for `axiomatic` claims:
- Official records (birth certificates, legal instruments, institutional filings)
- Primary biography or contemporaneous historical record
- Standard reference work of uncontested authority
- Mathematical or logical definition from a foundational text

Store one high-reliability source. A stack of corroboration is unnecessary
and obscures the canonical reference.

---

## 7. Kùzu Schema Additions

Add the following to the Cerebro graph store defined in `cerebro-kg-design.md`:

```python
# Source node table
conn.execute("""
  CREATE NODE TABLE Source(
    id               STRING PRIMARY KEY,
    label            STRING,
    source_type      STRING,
    reliability_tier STRING,
    uri              STRING,
    doi              STRING,
    isbn             STRING,
    arxiv_id         STRING,
    local_path       STRING,
    authors          STRING[],
    publication_year INT64,
    publisher        STRING,
    journal          STRING,
    peer_reviewed    BOOLEAN,
    retracted        BOOLEAN DEFAULT false,
    retraction_uri   STRING,
    retraction_date  STRING,
    accessed_at      STRING,
    added_at         STRING,
    notes            STRING
  )
""")

# Citation relationship table
conn.execute("""
  CREATE REL TABLE CitedBy(
    FROM Assertion TO Source,
    id               STRING,
    citation_type    STRING,
    page_or_section  STRING,
    quote            STRING,
    added_at         STRING
  )
""")
```

> Note: Kùzu relationship tables connect node types, not relationship types.
> The `CitedBy` relationship connects `Assertion` reified nodes to `Source` nodes.
> If your implementation stores assertions as edges rather than reified nodes,
> maintain the citation table in the quarantine SQLite store and join on promotion.

---

## 8. Citation Validation Query

Run periodically to surface integrity violations:

```python
def validate_citations(kuzu_conn):
    # Established claims with fewer than 2 sources
    result = kuzu_conn.execute("""
        MATCH (a:Entity)-[r:Assertion]->(b:Entity)
        WHERE r.confidence = 'established'
        WITH r, count {
            MATCH (r)-[:CitedBy]->(s:Source)
            WHERE s.retracted = false
        } AS source_count
        WHERE source_count < 2
        RETURN r.id, r.predicate, source_count
        ORDER BY source_count ASC
    """)
    print("Established claims with insufficient sources:")
    for row in result:
        print(f"  {row[0]}  predicate: {row[1]}  sources: {row[2]}")

    # Any claim citing only retracted sources
    result = kuzu_conn.execute("""
        MATCH (a:Entity)-[r:Assertion]->(b:Entity)
        WHERE r.confidence IN ['established', 'probable']
        WITH r, count {
            MATCH (r)-[:CitedBy]->(s:Source)
            WHERE s.retracted = false
        } AS live_sources
        WHERE live_sources = 0
        RETURN r.id, r.predicate, r.confidence
    """)
    print("\nClaims with no live (non-retracted) sources:")
    for row in result:
        print(f"  {row[0]}  predicate: {row[1]}  confidence: {row[2]}")
```

---

## 9. Retraction Cascade Query

When a source is marked retracted, identify affected edges for review:

```python
def retraction_cascade(kuzu_conn, source_id: str):
    """
    Returns all assertion edges that cite the given source,
    filtered to those where it is the only non-retracted source.
    These edges require human review of their confidence rating.
    """
    result = kuzu_conn.execute("""
        MATCH (src:Source {id: $source_id})
        MATCH (r:Assertion)-[:CitedBy]->(src)
        WITH r, count {
            MATCH (r)-[:CitedBy]->(other:Source)
            WHERE other.id <> $source_id
              AND other.retracted = false
        } AS other_live_sources
        RETURN r.id,
               r.predicate,
               r.confidence,
               other_live_sources,
               CASE WHEN other_live_sources = 0
                    THEN 'ORPHANED — review required'
                    ELSE 'other sources exist'
               END AS status
        ORDER BY other_live_sources ASC
    """, {"source_id": source_id})

    print(f"Retraction impact for source: {source_id}")
    for row in result:
        print(f"  edge: {row[0]}  [{row[2]}]  {row[4]}")
```

---

## 10. Quarantine Schema Additions

Add the following fields to the `extraction_candidates` table defined
in `extraction-design-schema.md` to carry citation data through the
review pipeline:

```sql
ALTER TABLE extraction_candidates ADD COLUMN
  source_reliability_tier TEXT;        -- reviewer-assigned on approval

ALTER TABLE extraction_candidates ADD COLUMN
  citation_type TEXT DEFAULT 'direct'; -- direct | indirect | refuting | contextual

ALTER TABLE extraction_candidates ADD COLUMN
  page_or_section TEXT;                -- optional page reference

ALTER TABLE extraction_candidates ADD COLUMN
  source_peer_reviewed BOOLEAN;        -- reviewer confirms or denies

ALTER TABLE extraction_candidates ADD COLUMN
  source_retracted BOOLEAN DEFAULT FALSE;
```

The reviewer who approves an `established`-tier edge must confirm that
the source meets the minimum requirements in Section 5 before promotion.
This is not a bureaucratic checkbox — it is the moment where the confidence
rating and the evidence are actually reconciled. An `established` edge whose
citations have not been validated against those requirements is not established:
it is merely labelled that way.

---

## 11. Integration Points

| Document                         | Integration                                          |
|----------------------------------|------------------------------------------------------|
| `cerebro-kg-design.md`           | Source nodes added to Kùzu node tables               |
| `cerebro-kg-design.md`           | `CitedBy` added to Kùzu relationship tables          |
| `extraction-design-schema.md`    | Quarantine schema extended — Section 10 above        |
| `extraction-design-schema.md`    | Promotion step creates Source node and CitedBy edge  |
| `cerebro-kg-design.md` edge schema | `evidence` field now points to Source node URI     |

---

*Document version: 2026-03-18 | System: Cerebro | Companions: cerebro-kg-design.md, extraction-design-schema.md*
