use domain::store::roots15;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

fn check<T: DeserializeOwned + Serialize>(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut doc: Value = serde_json::from_str(source)?;
    assert!(serde_json::from_value::<T>(doc.clone()).is_err());
    doc["schema_version"] = json!("1.5");
    let typed: T = serde_json::from_value(doc)?;
    let output = serde_json::to_value(typed)?;
    assert_eq!(output["schema_version"], "1.5");
    Ok(())
}

#[test]
fn unchanged_operational_roots_keep_distinct_version_markers()
-> Result<(), Box<dyn std::error::Error>> {
    check::<roots15::ConfigFile>(include_str!(
        "../../persistence/tests/fixtures/store/config.min.json"
    ))?;
    check::<roots15::WorkflowFile>(include_str!(
        "../../persistence/tests/fixtures/store/workflows.min.json"
    ))?;
    check::<roots15::LogFile>(include_str!(
        "../../persistence/tests/fixtures/store/log.min.json"
    ))?;
    check::<roots15::RagIndexFile>(include_str!(
        "../../persistence/tests/fixtures/store/rag_index.min.json"
    ))?;
    check::<roots15::ConversationFile>(include_str!(
        "../../persistence/tests/fixtures/store/conversations.min.json"
    ))?;
    check::<roots15::ToolManifestFile>(include_str!(
        "../../persistence/tests/fixtures/store/tool_manifest.min.json"
    ))?;
    Ok(())
}
