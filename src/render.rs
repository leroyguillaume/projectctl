use std::{collections::HashMap, fmt::Debug, fs::File, path::Path};

use liquid::{model::ScalarCow, to_object, Parser, ParserBuilder};
use liquid_core::{
    parser::FilterArguments, Display_filter, Filter, FilterReflection, ParseFilter, Runtime,
    ValueView,
};
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, debug_span};

use crate::{
    digest::hash_file,
    model::{Context, ProjectFile, ProjectMetadata, ProjectctlResult},
};

#[cfg_attr(test, mockall::automock)]
pub trait Renderer {
    fn render(
        &self,
        tpl: &Path,
        dest: &Path,
        vars: Value,
        ctx: &Context,
    ) -> ProjectctlResult<ProjectFile>;
}

pub struct LiquidRenderer {
    parser: Parser,
}

impl LiquidRenderer {
    pub fn init() -> ProjectctlResult<Self> {
        let parser = ParserBuilder::with_stdlib()
            .filter(JsonEncode)
            .filter(JsonEncodePretty)
            .build()?;
        Ok(Self { parser })
    }
}

impl Renderer for LiquidRenderer {
    fn render(
        &self,
        tpl: &Path,
        dest: &Path,
        vars: Value,
        ctx: &Context,
    ) -> ProjectctlResult<ProjectFile> {
        let span = debug_span!("render");
        let _enter = span.enter();
        debug!(path = %tpl.display(), "parsing template");
        let liquid_tpl = self.parser.parse_file(tpl)?;
        debug!(path = %dest.display(), "opening file");
        let mut file = File::create(dest)?;
        debug!(path = %dest.display(), "rendering template");
        let value = ObjectValue {
            env: &ctx.env,
            git: &ctx.git,
            project: &ctx.metadata,
            var: &vars,
        };
        let obj = to_object(&value)?;
        liquid_tpl.render_to(&mut file, &obj)?;
        Ok(ProjectFile {
            checksum: hash_file(dest)?,
            vars,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ObjectValue<'a> {
    env: &'a HashMap<String, String>,
    git: &'a HashMap<String, String>,
    project: &'a ProjectMetadata,
    var: &'a Value,
}

#[derive(Clone, FilterReflection)]
#[filter(
    name = "json_encode",
    description = "Encodes variable to JSON.",
    parsed(JsonEncodeFilter)
)]
struct JsonEncode;

impl ParseFilter for JsonEncode {
    fn parse(&self, _arguments: FilterArguments) -> liquid_core::Result<Box<dyn Filter>> {
        Ok(Box::new(JsonEncodeFilter(false)))
    }

    fn reflection(&self) -> &dyn FilterReflection {
        self
    }
}

#[derive(Clone, FilterReflection)]
#[filter(
    name = "json_encode_pretty",
    description = "Encodes variable to pretty JSON.",
    parsed(JsonEncodeFilter)
)]
struct JsonEncodePretty;

impl ParseFilter for JsonEncodePretty {
    fn parse(&self, _arguments: FilterArguments) -> liquid_core::Result<Box<dyn Filter>> {
        Ok(Box::new(JsonEncodeFilter(true)))
    }

    fn reflection(&self) -> &dyn FilterReflection {
        self
    }
}

#[derive(Debug, Default, Display_filter)]
#[name = "json_encode"]
struct JsonEncodeFilter(bool);

impl Filter for JsonEncodeFilter {
    fn evaluate(
        &self,
        input: &dyn ValueView,
        _runtime: &dyn Runtime,
    ) -> liquid_core::Result<liquid_core::Value> {
        let input = input.to_value();
        let json = if self.0 {
            serde_json::to_string_pretty(&input)
        } else {
            serde_json::to_string(&input)
        };
        let json = json.map_err(|err| liquid_core::Error::with_msg(err.to_string()))?;
        Ok(liquid_core::Value::Scalar(ScalarCow::new(json)))
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use crate::fs;

    use super::*;

    mod liquid_renderer {
        use super::*;

        #[test]
        fn render() {
            let tmp_dir = fs::test::create_tmp_dir();
            let dest = tmp_dir.path().join("dest");
            let vars = json!({
                "categories": ["category_1", "category_2"],
                "keywords": ["keyword_1", "keyword_2"],
            });
            let ctx = Context {
                env: Default::default(),
                git: HashMap::from_iter([
                    ("user_name".into(), "user".into()),
                    ("user_email".into(), "email".into()),
                ]),
                metadata: ProjectMetadata {
                    desc: Some("desc".into()),
                    name: "name".into(),
                    repo: Some("repository".into()),
                },
            };
            let renderer = LiquidRenderer::init().expect("failed to initialize renderer");
            let file = renderer
                .render(
                    Path::new("examples/templates/Cargo.toml.liquid"),
                    &dest,
                    vars.clone(),
                    &ctx,
                )
                .expect("failed to render template");
            let expected = ProjectFile {
                checksum: "b7a5a7b19aef367736dc72b899140777c9bedb56a2d5c1a001e5ff011eedb3c4".into(),
                vars,
            };
            assert_eq!(file, expected);
        }
    }
}
