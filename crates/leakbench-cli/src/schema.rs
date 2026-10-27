//! LeakBench report JSON Schema, kept as a string so `leakbench schema` can
//! emit it without a schema-deriving dependency.

pub const SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://leakbench.dev/schema/leakbench.report.v1.json",
  "title": "LeakBench report",
  "type": "object",
  "required": ["schema", "generated_at", "engine", "subject", "verdict", "score", "findings", "timeline", "facts", "disclaimer"],
  "properties": {
    "schema": { "const": "leakbench.report.v1" },
    "generated_at": { "type": "string", "description": "RFC 3339 UTC" },
    "engine": {
      "type": "object",
      "required": ["name", "version", "heuristics", "fact_sources", "network", "block_interval_secs"],
      "properties": {
        "name": { "type": "string" },
        "version": { "type": "string" },
        "heuristics": { "type": "integer", "minimum": 0 },
        "fact_sources": { "type": "array", "items": { "enum": ["fixture", "rpc", "simulated", "manual"] } },
        "network": { "type": "string" },
        "block_interval_secs": { "type": "integer", "minimum": 1 }
      }
    },
    "subject": {
      "type": "object",
      "required": ["label", "kind", "network", "tx_count", "has_wallet_facts"],
      "properties": {
        "label": { "type": "string" },
        "kind": { "enum": ["transaction", "wallet", "scenario"] },
        "network": { "type": "string" },
        "tx_count": { "type": "integer", "minimum": 0 },
        "first_height": { "type": ["integer", "null"] },
        "last_height": { "type": ["integer", "null"] },
        "description": { "type": ["string", "null"] },
        "has_wallet_facts": { "type": "boolean" }
      }
    },
    "verdict": { "enum": ["disqualified", "at-risk", "acceptable", "clean"] },
    "score": {
      "type": "object",
      "required": ["value", "grade", "verdict", "risk_points", "by_severity"],
      "properties": {
        "value": { "type": "integer", "minimum": 0, "maximum": 100 },
        "grade": { "type": "string" },
        "verdict": { "enum": ["disqualified", "at-risk", "acceptable", "clean"] },
        "risk_points": { "type": "number" },
        "by_severity": { "type": "object", "additionalProperties": { "type": "integer" } }
      }
    },
    "findings": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "title", "severity", "confidence", "category", "fatal", "summary", "evidence", "remediation"],
        "properties": {
          "id": { "type": "string", "pattern": "^LB-[0-9]{3}$" },
          "title": { "type": "string" },
          "severity": { "enum": ["info", "low", "medium", "high", "critical"] },
          "confidence": { "enum": ["low", "medium", "high"] },
          "category": { "enum": ["leak", "linkage", "fingerprint", "hygiene"] },
          "fatal": { "type": "boolean" },
          "summary": { "type": "string" },
          "evidence": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["label", "value"],
              "properties": {
                "label": { "type": "string" },
                "value": { "type": "string" },
                "detail": { "type": ["string", "null"] }
              }
            }
          },
          "remediation": { "type": "string" },
          "txids": { "type": "array", "items": { "type": "string" } },
          "references": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "timeline": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["txid", "label", "shape"],
        "properties": {
          "height": { "type": ["integer", "null"] },
          "time": { "type": ["integer", "null"] },
          "txid": { "type": "string" },
          "label": { "type": "string" },
          "shape": { "type": "string" },
          "severity": { "enum": ["info", "low", "medium", "high", "critical", null] }
        }
      }
    },
    "facts": {
      "type": "object",
      "required": ["transactions"],
      "properties": {
        "transactions": { "type": "array", "items": { "type": "object" } },
        "wallet": { "type": ["object", "null"] }
      }
    },
    "disclaimer": { "type": "string" }
  }
}"#;

#[cfg(test)]
mod tests {
    #[test]
    fn schema_is_valid_json() {
        let v: serde_json::Value = serde_json::from_str(super::SCHEMA).expect("valid json");
        assert_eq!(v["title"], "LeakBench report");
    }
}
