// This module contains helpers for rendering templates. These helpers can
// be registerd with the Handlebars library to assist in manipulating
// text at render time.

use handlebars::{Context, Handlebars, Helper, Output, RenderContext, RenderError};
use snafu::{OptionExt, ResultExt};

/// Potential errors during helper execution
mod error {
    use handlebars::RenderError;
    use snafu::Snafu;

    #[derive(Debug, Snafu)]
    #[snafu(visibility = "pub(super)")]
    pub(super) enum TemplateHelperError {
        #[snafu(display("No params provided to helper '{}' in template '{}'", helper, template))]
        NoParams { helper: String, template: String },

        // handlebars::JsonValue is a serde_json::Value, which implements
        // the 'Display' trait and should provide valuable context
        #[snafu(display(
            "Invalid (non-string) base64 template value: '{}' in template {}",
            value,
            template
        ))]
        InvalidTemplateValue {
            value: handlebars::JsonValue,
            template: String,
        },

        #[snafu(display(
            "Unable to base64 decode string '{}' in template '{}': '{}'",
            base64_string,
            template,
            source
        ))]
        Base64Decode {
            base64_string: String,
            template: String,
            source: base64::DecodeError,
        },

        #[snafu(display(
            "Invalid (non-utf8) output from decoded base64 in template '{}': '{}'",
            template,
            source
        ))]
        InvalidUTF8 {
            template: String,
            source: std::str::Utf8Error,
        },

        #[snafu(display("Unable to write template: '{}': '{}'", template, source))]
        TemplateWrite {
            template: String,
            source: std::io::Error,
        },
    }

    // Handlebars helpers are required to return a RenderError.
    // Implement "From" for TemplateHelperError.
    impl From<TemplateHelperError> for RenderError {
        fn from(e: TemplateHelperError) -> RenderError {
            RenderError::with(e)
        }
    }
}

/// `base64_decode` decodes base64 encoded text at template render time.
/// It takes a single variable as a parameter: {{base64_decode var}}
pub fn base64_decode(
    helper: &Helper,
    _: &Handlebars,
    _: &Context,
    renderctx: &mut RenderContext,
    out: &mut Output,
) -> Result<(), RenderError> {
    // To give context to our errors, get the template name.
    // In the context of thar-be-settings, all of our templates have
    // registered names, which means that the get_root_template_name()
    // call should return an intelligible and valid name.
    // To protect us in the unlikely event a template was registered
    // without a name, we return the text "dynamic template"
    let template_name = renderctx
        .get_root_template_name()
        .map(|i| i.to_string())
        .unwrap_or("dynamic template".to_string());

    // Get the resolved key out of the template (param(0)). value() returns
    // a serde_json::Value
    let base64_value = helper
        .param(0)
        .map(|v| v.value())
        .context(error::NoParams {
            helper: helper.name().to_string(),
            template: template_name.to_owned(),
        })?;

    // Create an &str from the serde_json::Value
    let base64_str = base64_value.as_str().context(error::InvalidTemplateValue {
        value: base64_value.to_owned(),
        template: template_name.to_owned(),
    })?;

    // Base64 decode the &str
    let decoded_bytes = base64::decode(&base64_str).context(error::Base64Decode {
        base64_string: base64_str.to_string(),
        template: template_name.to_owned(),
    })?;

    // Create a valid utf8 str
    let decoded = std::str::from_utf8(&decoded_bytes).context(error::InvalidUTF8 {
        template: template_name.to_owned(),
    })?;

    // Write the string out to the template
    out.write(decoded).context(error::TemplateWrite {
        template: template_name.to_owned(),
    })?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn renders_decoded_base64() {
        let mut registry = Handlebars::new();
        registry.register_helper("base64_decode", Box::new(base64_decode));

        let result = registry
            .render_template("{{base64_decode var}}", &json!({"var": "SGk="}))
            .unwrap();

        assert_eq!(result, "Hi")
    }

    #[test]
    fn does_not_render_invalid_base64() {
        let mut registry = Handlebars::new();
        registry.register_helper("base64_decode", Box::new(base64_decode));

        assert!(registry
            .render_template("{{base64_decode var}}", &json!({"var": "invalid_"}))
            .is_err());
    }
}
