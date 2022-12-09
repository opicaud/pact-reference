//! Functions to apply generators to body contents

use std::collections::HashMap;

use pact_plugin_driver::catalogue_manager::find_content_generator;
use serde_json::Value;
use tracing::{debug, error, warn};

use pact_models::bodies::OptionalBody;
use pact_models::content_types::ContentType;
use pact_models::generators::{ContentTypeHandler, Generator, GeneratorTestMode, JsonHandler, VariantMatcher};
use pact_models::path_exp::DocPath;
use pact_models::plugins::PluginData;
use pact_models::xml_utils::parse_bytes;

use crate::generators::XmlHandler;

/// Apply the generators to the body, returning a new body
pub async fn generators_process_body(
  mode: &GeneratorTestMode,
  body: &OptionalBody,
  content_type: Option<ContentType>,
  context: &HashMap<&str, Value>,
  generators: &HashMap<DocPath, Generator>,
  matcher: &(dyn VariantMatcher + Send + Sync),
  plugin_data: &Vec<PluginData>,
  interaction_data: &HashMap<String, HashMap<String, Value>>
) -> anyhow::Result<OptionalBody> {
  match content_type {
    Some(content_type) => if content_type.is_json() {
      debug!("apply_body_generators: JSON content type");
      let result: Result<Value, serde_json::Error> = serde_json::from_slice(&body.value().unwrap_or_default());
      match result {
        Ok(val) => {
          let mut handler = JsonHandler { value: val };
          Ok(handler.process_body(generators, mode, context, &matcher.boxed()).unwrap_or_else(|err| {
            error!("Failed to generate the body: {}", err);
            body.clone()
          }))
        },
        Err(err) => {
          error!("Failed to parse the body, so not applying any generators: {}", err);
          Ok(body.clone())
        }
      }
    } else if content_type.is_xml() {
      debug!("apply_body_generators: XML content type");
      match parse_bytes(&body.value().unwrap_or_default()) {
        Ok(val) => {
          let mut handler = XmlHandler { value: val.as_document() };
          Ok(handler.process_body(generators, mode, context, &matcher.boxed()).unwrap_or_else(|err| {
            error!("Failed to generate the body: {}", err);
            body.clone()
          }))
        },
        Err(err) => {
          error!("Failed to parse the body, so not applying any generators: {}", err);
          Ok(body.clone())
        }
      }
    } else if let Some(content_generator) = find_content_generator(&content_type) {
      debug!("apply_body_generators: Found a content generator from a plugin");
      let generators = generators.iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
      content_generator.generate_content(&content_type, &generators, body, plugin_data, interaction_data, context).await
    } else {
      warn!("Unsupported content type {} - Generators only support JSON and XML", content_type);
      Ok(body.clone())
    },
    _ => Ok(body.clone())
  }
}

#[cfg(test)]
mod tests {
  use expectest::prelude::*;
  use maplit::hashmap;

  use pact_models::bodies::OptionalBody;
  use pact_models::content_types::{JSON, TEXT};
  use pact_models::generators::GeneratorTestMode;

  use super::generators_process_body;
  use crate::DefaultVariantMatcher;

  #[tokio::test]
  async fn apply_generator_to_empty_body_test() {
    expect!(generators_process_body(&GeneratorTestMode::Provider, &OptionalBody::Empty,
      Some(TEXT.clone()), &hashmap!{}, &hashmap!{}, &DefaultVariantMatcher{}, &vec![], &hashmap!{})
      .await.unwrap()).to(be_equal_to(OptionalBody::Empty));
    expect!(generators_process_body(&GeneratorTestMode::Provider, &OptionalBody::Null,
      Some(TEXT.clone()), &hashmap!{}, &hashmap!{}, &DefaultVariantMatcher{}, &vec![], &hashmap!{})
      .await.unwrap()).to(be_equal_to(OptionalBody::Null));
    expect!(generators_process_body(&GeneratorTestMode::Provider, &OptionalBody::Missing,
      Some(TEXT.clone()), &hashmap!{}, &hashmap!{}, &DefaultVariantMatcher{}, &vec![], &hashmap!{})
      .await.unwrap()).to(be_equal_to(OptionalBody::Missing));
  }

  #[tokio::test]
  async fn do_not_apply_generators_if_there_are_no_body_generators() {
    let body = OptionalBody::Present("{\"a\":100,\"b\":\"B\"}".into(), Some(JSON.clone()), None);
    expect!(generators_process_body(&GeneratorTestMode::Provider, &body, Some(JSON.clone()),
    &hashmap!{}, &hashmap!{}, &DefaultVariantMatcher{}, &vec![], &hashmap!{}).await.unwrap()).to(
      be_equal_to(body));
  }

  #[tokio::test]
  async fn apply_generator_to_text_body_test() {
    let body = OptionalBody::Present("some text".into(), None, None);
    expect!(generators_process_body(&GeneratorTestMode::Provider, &body, Some(TEXT.clone()),
    &hashmap!{}, &hashmap!{}, &DefaultVariantMatcher{}, &vec![], &hashmap!{}).await.unwrap()).to(be_equal_to(body));
  }
}
