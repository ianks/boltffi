use crate::{
    cli::{CliError, Result},
    commands::generate::generator::{GenerateRequest, LanguageGenerator, ScanPointerWidth},
    config::Target,
};
use boltffi_bindgen::render::ruby::{RubyEmitter, RubyLowerer};

pub struct RubyGenerator;

impl RubyGenerator {
    #[cfg(test)]
    pub(crate) fn generate_from_source_directory(
        config: &crate::config::Config,
        output_override: Option<std::path::PathBuf>,
        source_directory: &std::path::Path,
        crate_name: &str,
    ) -> Result<()> {
        let request = GenerateRequest::new(
            config,
            output_override,
            crate::commands::generate::generator::SourceCrate::new(source_directory, crate_name),
        );

        Self::generate(&request)
    }
}

impl LanguageGenerator for RubyGenerator {
    const TARGET: Target = Target::Ruby;

    fn generate(request: &GenerateRequest<'_>) -> Result<()> {
        if !request.config().is_ruby_enabled() {
            return Err(CliError::CommandFailed {
                command: "targets.ruby.enabled = false".to_string(),
                status: None,
            });
        }

        let output_directory = request
            .output_override()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| request.config().ruby_output());

        request.ensure_output_directory(&output_directory)?;

        let lowered_crate = request.lowered_crate(ScanPointerWidth::Flexible)?;

        let module =
            RubyLowerer::new(&lowered_crate.ffi_contract, &lowered_crate.abi_contract).lower();

        let sources = RubyEmitter::new(
            request.config().ruby_ractor_safe(),
            request.config().ruby_gem_name(),
        )
        .emit(&module);

        for file in &sources.files {
            let path = output_directory.join(&file.relative_path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| CliError::CommandFailed {
                    command: format!("create directory: {}", e),
                    status: None,
                })?;
            }
            request.write_output(&path, &file.contents)?;
        }

        Ok(())
    }
}
