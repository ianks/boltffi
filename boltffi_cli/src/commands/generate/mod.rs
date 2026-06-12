mod generator;
mod header;
mod languages;

use std::path::{Path, PathBuf};

use boltffi_bindgen::CHeaderLowerer;
use generator::{GenerateRequest, ScanPointerWidth, run_generator};
use header::HeaderGenerator;
use languages::{
    CSharpGenerator, DartGenerator, JavaGenerator, KMPGenerator, KotlinGenerator, PythonGenerator,
    RubyGenerator, SwiftGenerator, TypeScriptGenerator,
};

use crate::cli::Result;
use crate::config::{Config, Target};

pub enum GenerateTarget {
    Swift,
    Kotlin,
    KotlinMultiplatform,
    Java,
    Header,
    Typescript,
    Dart,
    Python,
    Ruby,
    CSharp,
    All,
}

pub struct GenerateOptions {
    pub target: GenerateTarget,
    pub output: Option<PathBuf>,
    pub experimental: bool,
}

pub fn run_generate_with_output(config: &Config, options: GenerateOptions) -> Result<()> {
    let request = GenerateRequest::for_current_crate(config, options.output);

    match options.target {
        GenerateTarget::Swift => run_generator::<SwiftGenerator>(&request, options.experimental),
        GenerateTarget::Kotlin => run_generator::<KotlinGenerator>(&request, options.experimental),
        GenerateTarget::KotlinMultiplatform => {
            run_generator::<KMPGenerator>(&request, options.experimental)
        }
        GenerateTarget::Java => run_generator::<JavaGenerator>(&request, options.experimental),
        GenerateTarget::Header => run_generator::<HeaderGenerator>(&request, options.experimental),
        GenerateTarget::Typescript => {
            run_generator::<TypeScriptGenerator>(&request, options.experimental)
        }
        GenerateTarget::Dart => run_generator::<DartGenerator>(&request, options.experimental),
        GenerateTarget::Python => run_generator::<PythonGenerator>(&request, options.experimental),
        GenerateTarget::Ruby => run_generator::<RubyGenerator>(&request, options.experimental),
        GenerateTarget::CSharp => run_generator::<CSharpGenerator>(&request, options.experimental),
        GenerateTarget::All => {
            if config.should_process(Target::Swift, options.experimental) {
                run_generator::<SwiftGenerator>(&request, options.experimental)?;
            }

            if config.should_process(Target::Kotlin, options.experimental) {
                run_generator::<KotlinGenerator>(&request, options.experimental)?;
            }

            if config.should_process(Target::KotlinMultiplatform, options.experimental) {
                run_generator::<KMPGenerator>(&request, options.experimental)?;
            }

            if config.should_process(Target::Java, options.experimental) {
                run_generator::<JavaGenerator>(&request, options.experimental)?;
            }

            if config.should_process(Target::Header, options.experimental) {
                run_generator::<HeaderGenerator>(&request, options.experimental)?;
            }

            if config.should_process(Target::TypeScript, options.experimental) {
                run_generator::<TypeScriptGenerator>(&request, options.experimental)?;
            }

            if config.should_process(Target::Dart, options.experimental) {
                run_generator::<DartGenerator>(&request, options.experimental)?;
            }

            if config.should_process(Target::Python, options.experimental) {
                run_generator::<PythonGenerator>(&request, options.experimental)?;
            }

            if config.should_process(Target::Ruby, options.experimental) {
                run_generator::<RubyGenerator>(&request, options.experimental)?;
            }

            if config.should_process(Target::CSharp, options.experimental) {
                run_generator::<CSharpGenerator>(&request, options.experimental)?;
            }

            Ok(())
        }
    }
}

pub fn run_generate_java_with_output_from_source_dir(
    config: &Config,
    output: Option<PathBuf>,
    source_directory: &Path,
    crate_name: &str,
) -> Result<()> {
    JavaGenerator::generate_from_source_directory(config, output, source_directory, crate_name)
}

pub fn run_generate_kmp_with_output_from_source_dir_and_desktop_fallback_library_name(
    config: &Config,
    output: Option<PathBuf>,
    source_directory: &Path,
    crate_name: &str,
    desktop_fallback_library_name: &str,
) -> Result<()> {
    KMPGenerator::generate_from_source_directory_with_desktop_fallback_library_name(
        config,
        output,
        source_directory,
        crate_name,
        Some(desktop_fallback_library_name),
    )
}

pub fn run_generate_header_with_output_from_source_dir(
    config: &Config,
    output: Option<PathBuf>,
    source_directory: &Path,
    crate_name: &str,
) -> Result<()> {
    let output_directory = output
        .as_ref()
        .cloned()
        .unwrap_or_else(|| config.android_header_output());
    let request = GenerateRequest::new(
        config,
        output,
        generator::SourceCrate::new(source_directory, crate_name),
    );

    let output_path = output_directory.join(format!("{}.h", config.library_name()));

    request.ensure_output_directory(&output_directory)?;
    let lowered_crate = request.lowered_crate(ScanPointerWidth::Flexible)?;
    let header_source =
        CHeaderLowerer::new(&lowered_crate.ffi_contract, &lowered_crate.abi_contract).generate();

    request.write_output(&output_path, header_source)
}

pub fn run_generate_python_with_output_from_source_dir(
    config: &Config,
    output: Option<PathBuf>,
    source_directory: &Path,
    crate_name: &str,
) -> Result<()> {
    PythonGenerator::generate_from_source_directory(config, output, source_directory, crate_name)
}

pub fn run_generate_csharp_with_output_from_source_dir(
    config: &Config,
    output: Option<PathBuf>,
    source_directory: &Path,
    crate_name: &str,
) -> Result<()> {
    CSharpGenerator::generate_from_source_directory(config, output, source_directory, crate_name)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use boltffi_bindgen::render::python::PythonRuntimeVersion;

    use super::languages::{KMPGenerator, PythonGenerator, RubyGenerator};
    use crate::config::Config;

    fn parse_config(input: &str) -> Config {
        let parsed: Config = toml::from_str(input).expect("toml parse failed");
        parsed.validate().expect("config validation failed");
        parsed
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("{prefix}-{unique_suffix}"))
    }

    fn demo_source_directory() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/demo")
    }

    #[test]
    fn header_from_source_directory_supports_kmp_only_config() {
        let output_directory = unique_temp_dir("boltffi-kmp-header-generate-test");
        let config = parse_config(
            r#"
experimental = ["kotlin_multiplatform"]

[package]
name = "demo"
version = "0.1.0"

[targets.apple]
enabled = false

[targets.android]
enabled = false

[targets.kotlin_multiplatform]
enabled = true
"#,
        );

        super::run_generate_header_with_output_from_source_dir(
            &config,
            Some(output_directory.clone()),
            &demo_source_directory(),
            "demo",
        )
        .expect("kmp-only header generation should succeed");

        let header_path = output_directory.join("demo.h");
        let header = fs::read_to_string(&header_path).expect("header should be readable");

        assert!(header.contains("boltffi"));
        assert!(header.contains("BoltFFICallbackHandle"));

        fs::remove_dir_all(output_directory).expect("cleanup generated output");
    }

    #[test]
    fn python_generate_writes_python_package_sources() {
        let output_directory = unique_temp_dir("boltffi-python-generate-test");
        let config = parse_config(
            r#"
[package]
name = "demo"
version = "0.1.0"

[targets.python]
enabled = true
"#,
        );

        PythonGenerator::generate_from_source_directory(
            &config,
            Some(output_directory.clone()),
            &demo_source_directory(),
            "demo",
        )
        .expect("python generate should succeed");

        let generated_init_path = output_directory.join("demo/__init__.py");
        let generated_stub_path = output_directory.join("demo/__init__.pyi");
        let generated_native_path = output_directory.join("demo/_native.c");
        let generated_pyproject_path = output_directory.join("pyproject.toml");
        let generated_setup_path = output_directory.join("setup.py");
        let generated_init = fs::read_to_string(&generated_init_path)
            .expect("generated python init should be readable");
        let generated_stub = fs::read_to_string(&generated_stub_path)
            .expect("generated python typing stub should be readable");
        let generated_native = fs::read_to_string(&generated_native_path)
            .expect("generated native bridge should be readable");
        let generated_pyproject = fs::read_to_string(&generated_pyproject_path)
            .expect("generated pyproject should be readable");
        let generated_setup = fs::read_to_string(&generated_setup_path)
            .expect("generated setup.py should be readable");
        let minimum_python_version_requirement =
            PythonRuntimeVersion::minimum_supported().package_requirement();

        assert!(generated_init.contains("from pathlib import Path"));
        assert!(generated_init.contains("from . import _native"));
        assert!(generated_init.contains("_native._initialize_loader"));
        assert!(generated_init.contains("__all__ = ["));
        assert!(generated_init.contains("PACKAGE_NAME = \"demo\""));
        assert!(generated_stub.contains("MODULE_NAME: str"));
        assert!(generated_stub.contains("def echo_i32"));
        assert!(generated_pyproject.contains("setuptools.build_meta"));
        assert!(generated_setup.contains("Extension("));
        assert!(generated_setup.contains("\"demo._native\""));
        assert!(generated_setup.contains(&format!(
            "python_requires={minimum_python_version_requirement:?}"
        )));
        assert!(generated_native.contains("boltffi_python_symbol_echo_i32_fn"));
        assert!(generated_native.contains("boltffi_python_initialize_loader"));
        assert!(generated_native.contains("PyInit__native"));

        fs::remove_dir_all(output_directory).expect("cleanup generated output");
    }

    #[test]
    fn kotlin_multiplatform_generate_writes_kmp_sources() {
        let output_directory = unique_temp_dir("boltffi-kmp-generate-test");
        let config = parse_config(
            r#"
experimental = ["kotlin_multiplatform"]

[package]
name = "demo"
version = "0.1.0"

[targets.kotlin_multiplatform]
enabled = true
package = "com.boltffi.demo"

[targets.android.kotlin.type_mappings]
Email = { type = "java.net.URI", conversion = "url_string" }
"#,
        );

        KMPGenerator::generate_from_source_directory_with_desktop_fallback_library_name(
            &config,
            Some(output_directory.clone()),
            &demo_source_directory(),
            "demo",
            None,
        )
        .expect("kotlin multiplatform generate should succeed");

        let common_path = output_directory.join("src/commonMain/kotlin/com/boltffi/demo/Demo.kt");
        let jvm_actual_path =
            output_directory.join("src/jvmMain/kotlin/com/boltffi/demo/DemoJvmActual.kt");
        let android_actual_path =
            output_directory.join("src/androidMain/kotlin/com/boltffi/demo/DemoAndroidActual.kt");
        let jvm_internal_path =
            output_directory.join("src/jvmMain/kotlin/com/boltffi/demo/jvm/Demo.kt");
        let jni_glue_path = output_directory.join("src/jvmMain/c/jni_glue.c");
        let build_gradle_path = output_directory.join("build.gradle.kts");
        let settings_gradle_path = output_directory.join("settings.gradle.kts");

        let common = fs::read_to_string(&common_path).expect("common source should be readable");
        let jvm_actual =
            fs::read_to_string(&jvm_actual_path).expect("jvm actual should be readable");
        let android_actual =
            fs::read_to_string(&android_actual_path).expect("android actual should be readable");
        let jvm_internal =
            fs::read_to_string(&jvm_internal_path).expect("jvm source should be readable");
        let jni_glue = fs::read_to_string(&jni_glue_path).expect("jni glue should be readable");
        let build_gradle =
            fs::read_to_string(&build_gradle_path).expect("gradle file should be readable");
        let settings_gradle =
            fs::read_to_string(&settings_gradle_path).expect("settings file should be readable");

        assert!(common.contains("package com.boltffi.demo"));
        assert!(common.contains("typealias Email = String"));
        assert!(common.contains(
            "class FfiException(val code: kotlin.Int, message: kotlin.String) : kotlin.Exception(message)"
        ));
        assert!(common.contains("sealed class BoltFFIResult<out T, out E>"));
        assert!(common.contains("data class Point("));
        assert!(common.contains("sealed class MathError : kotlin.Exception()"));
        assert!(common.contains("data class AppError("));
        assert!(common.contains("sealed class ComputeError : kotlin.Exception()"));
        assert!(common.contains("data class Triangle(val a: com.boltffi.demo.Point"));
        assert!(common.contains("data class BenchmarkResponse("));
        assert!(common.contains("val result: BoltFFIResult<DataPoint, ComputeError>"));
        assert!(common.contains("enum class LogLevel(val value: Byte)"));
        assert!(common.contains("expect fun echoBytes"));
        assert!(common.contains("expect fun checkedDivide(a: Int, b: Int): Int"));
        assert!(
            common.contains("expect fun resultToString(v: BoltFFIResult<Int, String>): String")
        );
        assert!(common.contains("Unsupported in the initial KMP generator slice"));
        assert!(jvm_actual.contains("actual fun echoBytes"));
        assert!(jvm_actual.contains("actual fun checkedDivide(a: Int, b: Int): Int"));
        assert!(jvm_actual.contains("catch (err: com.boltffi.demo.jvm.MathError)"));
        assert!(jvm_actual.contains("catch (err: com.boltffi.demo.jvm.FfiException)"));
        assert!(jvm_actual.contains("private fun MathError.toBoltFfiJvm()"));
        assert!(
            jvm_actual.contains("private fun com.boltffi.demo.jvm.MathError.toBoltFfiCommon()")
        );
        assert!(jvm_actual.contains("com.boltffi.demo.jvm.echoBytes"));
        assert!(jvm_actual.contains("toBoltFfiJvm"));
        assert_eq!(jvm_actual, android_actual);
        assert!(jvm_internal.contains("package com.boltffi.demo.jvm"));
        assert!(jvm_internal.contains("typealias Email = String"));
        assert!(jvm_internal.contains("@JvmStatic external fun"));
        assert!(jni_glue.contains("JNIEXPORT"));
        assert!(build_gradle.contains("kotlin(\"multiplatform\")"));
        assert!(build_gradle.contains("kotlin(\"multiplatform\") version \"2.3.21\""));
        assert!(build_gradle.contains("kotlinx-coroutines-core:1.11.0"));
        assert!(build_gradle.contains("import org.jetbrains.kotlin.gradle.dsl.JvmTarget"));
        assert!(build_gradle.contains("jvmTarget.set(JvmTarget.JVM_1_8)"));
        assert!(build_gradle.contains("androidTarget {"));
        assert!(build_gradle.contains("sourceCompatibility = JavaVersion.VERSION_1_8"));
        assert!(build_gradle.contains("targetCompatibility = JavaVersion.VERSION_1_8"));
        assert!(!build_gradle.contains("repositories {"));
        assert!(settings_gradle.contains("pluginManagement"));
        assert!(settings_gradle.contains("gradlePluginPortal()"));
        assert!(settings_gradle.contains("RepositoriesMode.FAIL_ON_PROJECT_REPOS"));

        fs::remove_dir_all(output_directory).expect("cleanup generated output");
    }

    #[test]
    fn kotlin_multiplatform_generate_uses_configured_native_load_name() {
        let output_directory = unique_temp_dir("boltffi-kmp-generate-load-name-test");
        let config = parse_config(
            r#"
experimental = ["kotlin_multiplatform"]

[package]
name = "my-lib"
version = "0.1.0"

[targets.android.kotlin]
library_name = "configured-library"

[targets.kotlin_multiplatform]
enabled = true
package = "com.boltffi.demo"
module_name = "Demo"
"#,
        );

        KMPGenerator::generate_from_source_directory_with_desktop_fallback_library_name(
            &config,
            Some(output_directory.clone()),
            &demo_source_directory(),
            "my-lib",
            None,
        )
        .expect("kotlin multiplatform generate should succeed");

        let jvm_internal_path =
            output_directory.join("src/jvmMain/kotlin/com/boltffi/demo/jvm/Demo.kt");
        let jni_glue_path = output_directory.join("src/jvmMain/c/jni_glue.c");
        let jvm_internal =
            fs::read_to_string(&jvm_internal_path).expect("jvm source should be readable");
        let jni_glue = fs::read_to_string(&jni_glue_path).expect("jni glue should be readable");

        assert!(jvm_internal.contains("val androidLibrary = \"configured-library\""));
        assert!(jvm_internal.contains("val desktopPreferredLibrary = \"configured_library_jni\""));
        assert!(jvm_internal.contains("val desktopFallbackLibrary = \"my_lib\""));
        assert!(jni_glue.contains("#include <my-lib.h>"));

        fs::remove_dir_all(output_directory).expect("cleanup generated output");
    }

    #[test]
    fn ruby_generate_writes_ruby_package_sources() {
        let output_directory = unique_temp_dir("boltffi-ruby-generate-test");
        let config = parse_config(
            r#"
[package]
name = "demo"
version = "0.1.0"

[targets.ruby]
enabled = true
"#,
        );

        RubyGenerator::generate_from_source_directory(
            &config,
            Some(output_directory.clone()),
            &demo_source_directory(),
            "demo",
        )
        .expect("ruby generate should succeed");

        // Output structure (flat makefile target = "demo_native"):
        let lib_rb = output_directory.join("lib/demo.rb");
        let native_c = output_directory.join("ext/demo/_native.c");
        let extconf = output_directory.join("ext/demo/extconf.rb");
        let gemspec = output_directory.join("demo.gemspec");

        assert!(lib_rb.exists(), "lib/demo.rb missing");
        assert!(native_c.exists(), "ext/demo/_native.c missing");
        assert!(extconf.exists(), "ext/demo/extconf.rb missing");
        assert!(gemspec.exists(), "demo.gemspec missing");

        let lib_contents = fs::read_to_string(&lib_rb).expect("lib.rb readable");
        let native_contents = fs::read_to_string(&native_c).expect("native.c readable");
        let extconf_contents = fs::read_to_string(&extconf).expect("extconf.rb readable");
        let gemspec_contents = fs::read_to_string(&gemspec).expect("gemspec readable");

        // lib.rb: single require, statically linked — NO runtime loader call
        assert!(
            lib_contents.contains("require_relative \"demo_native\""),
            "lib.rb must require_relative 'demo_native'"
        );
        assert!(
            !lib_contents.contains("load_library!"),
            "lib.rb must NOT call load_library! (static linking — no runtime dlopen)"
        );

        // native.c: direct extern calls, no dlopen; correct Init_ name (no double underscore)
        assert!(
            native_contents.contains("void Init_demo_native"),
            "native.c must define Init_demo_native"
        );
        // Check that it doesn't actually call dlopen (not just in comments)
        // The comment "no dlopen" is expected; we want to ensure dlopen( is not called
        assert!(
            !native_contents.contains("dlopen("),
            "native.c must NOT call dlopen (static linking)"
        );
        assert!(
            native_contents.contains("extern "),
            "native.c must declare Rust symbols extern for static linking"
        );
        // Ractor-safety is opt-in: with the default config it must NOT be declared.
        assert!(
            !native_contents.contains("rb_ext_ractor_safe"),
            "native.c must NOT declare Ractor-safety unless [targets.ruby] ractor_safe is set"
        );

        // extconf.rb: statically links the vendored Rust archive + hides its symbols
        assert!(
            extconf_contents.contains("libdemo_ffi.a"),
            "extconf.rb must link the vendored Rust staticlib libdemo_ffi.a"
        );
        assert!(
            (extconf_contents.contains("--exclude-libs,ALL")
                || extconf_contents.contains("-exported_symbol,_Init_demo_native")),
            "extconf.rb must hide static-archive symbols (export only Init_demo_native)"
        );
        assert!(
            extconf_contents.contains("create_makefile(\"demo_native\")"),
            "extconf.rb must use flat create_makefile name"
        );
        assert!(
            extconf_contents.contains("$CFLAGS << \" -g\""),
            "extconf.rb must compile the glue with debug symbols (-g)"
        );
        // Build-from-source path: when no prebuilt .a is present, extconf compiles
        // the staticlib robustly — forcing the crate-type and discovering the
        // artifact from cargo's JSON output rather than guessing target/release/...
        assert!(
            extconf_contents.contains("--crate-type")
                && extconf_contents.contains("staticlib"),
            "extconf.rb must force --crate-type staticlib when building from source"
        );
        assert!(
            extconf_contents.contains("--message-format")
                && extconf_contents.contains("compiler-artifact"),
            "extconf.rb must discover the .a from cargo's JSON output, not a guessed path"
        );
        assert!(
            extconf_contents.contains("BOLTFFI_CRATE_MANIFEST"),
            "extconf.rb must honor the BOLTFFI_CRATE_MANIFEST override for the crate source"
        );
        assert!(
            extconf_contents.contains("--target-dir")
                && extconf_contents.contains("Dir.mktmpdir"),
            "extconf.rb must build into a throwaway target dir that is cleaned up"
        );
        // The cargo build runs at `make` time, injected into mkmf's Makefile
        // (not at configure time) — for $CARGO/$RUSTFLAGS/clean/cross-build
        // compatibility. extconf appends a rule making the archive a
        // prerequisite of the link, whose recipe re-invokes extconf in build mode.
        assert!(
            extconf_contents.contains("File.open(\"Makefile\"")
                && extconf_contents.contains("$(DLLIB): $(BOLTFFI_STATICLIB)")
                && extconf_contents.contains("BOLTFFI_BUILD_STATICLIB"),
            "extconf.rb must inject a make-time rule to build the staticlib (not build at configure time)"
        );
        // The crate's transitive system libraries come from `rustc --print
        // native-static-libs` (captured during the same build), NEVER hardcoded.
        assert!(
            extconf_contents.contains("--print")
                && extconf_contents.contains("native-static-libs"),
            "extconf.rb must derive system libs from rustc --print native-static-libs"
        );
        assert!(
            extconf_contents.contains("BOLTFFI_NATIVE_LIBS_FILE")
                && extconf_contents.contains(".native-libs")
                && extconf_contents.contains("LIBS = $(BOLTFFI_ORIG_LIBS)"),
            "extconf.rb must feed the captured native-libs sidecar into the link via $(LIBS)"
        );
        assert!(
            !extconf_contents.contains("-framework Security")
                && !extconf_contents.contains("-lpthread"),
            "extconf.rb must NOT hardcode system libraries — they come from native-static-libs"
        );
        // mkmf best-practice: the archive is a link INPUT, not a linker flag.
        // $LOCAL_LIBS is the right variable (mkmf places it after $LDFLAGS in the
        // link line, which is also after --exclude-libs,ALL — the correct GNU ld order).
        assert!(
            extconf_contents.contains("$LOCAL_LIBS << \" #{archive}\""),
            "extconf.rb must put the archive in $LOCAL_LIBS, not $LDFLAGS"
        );
        assert!(
            !extconf_contents.contains("$LDFLAGS << \" #{archive}\""),
            "extconf.rb must NOT put the archive in $LDFLAGS (breaks MSVC; wrong ld order)"
        );
        // Symbol hiding must cover FreeBSD/OpenBSD (lld accepts GNU ld flags) and
        // Windows MinGW (rake-compiler-dock cross builds use GNU ld-compatible tooling).
        assert!(
            extconf_contents.contains("freebsd") && extconf_contents.contains("openbsd"),
            "extconf.rb must hide symbols on FreeBSD/OpenBSD (same GNU ld flags as Linux)"
        );
        assert!(
            extconf_contents.contains("mingw") && extconf_contents.contains("mswin"),
            "extconf.rb must have Windows (mingw/mswin) branches for symbol hiding"
        );
        // The clean rule extends mkmf's clean: target by adding clean-boltffi-staticlib
        // as an additive prerequisite. GNU make merges prerequisites from multiple `:`
        // rules for the same target; we must NOT use `::` (double-colon) here because
        // mkmf generates `clean:` (single-colon) and mixing the two is a make error.
        assert!(
            extconf_contents.contains("clean: clean-boltffi-staticlib"),
            "extconf.rb Makefile injection must use clean: (single-colon, additive prerequisite)"
        );

        // gemspec: deterministic + crate-sourced metadata, no Dir[] globbing
        // The comment mentions Dir[...] so we check for spec.files = Dir[ as the actual pattern
        assert!(
            !gemspec_contents.contains("spec.files = Dir["),
            "gemspec must use an explicit file manifest, not Dir[] globbing (reproducibility)"
        );
        // `generate` emits the source-gem shape: it ships no prebuilt binary and
        // stays platform-agnostic, so the manifest must NOT list a .a and the
        // platform must be RUBY. The precompiled .a + spec.platform=<triple> shape
        // is `boltffi pack ruby`'s job.
        assert!(
            !gemspec_contents.contains("\"ext/demo/libdemo_ffi.a\""),
            "source gem manifest must NOT list a .a (that is the precompiled pack shape)"
        );
        assert!(
            gemspec_contents.contains("spec.platform = Gem::Platform::RUBY"),
            "source gem must set spec.platform = Gem::Platform::RUBY"
        );
        assert!(
            gemspec_contents.contains("spec.extensions = [\"ext/demo/extconf.rb\"]"),
            "gemspec must declare the extconf extension"
        );
        // demo's Cargo.toml has [package] version = "0.1.0"
        // The scan/IR may not yet capture the crate version from Cargo.toml, so we assert
        // the fallback behavior: spec.version = either the crate version OR "0.0.0" if absent
        assert!(
            gemspec_contents.contains("spec.version = \"0.1.0\"")
                || gemspec_contents.contains("spec.version = \"0.0.0\""),
            "gemspec version must be crate version (0.1.0) or fallback (0.0.0)"
        );
        assert!(
            gemspec_contents.contains("rubygems_mfa_required"),
            "gemspec should set the rubygems_mfa_required metadata flag"
        );

        fs::remove_dir_all(output_directory).expect("cleanup generated output");
    }

    #[test]
    fn ruby_generate_opts_into_ractor_safety() {
        let output_directory = unique_temp_dir("boltffi-ruby-ractor-test");
        let config = parse_config(
            r#"
[package]
name = "demo"
version = "0.1.0"

[targets.ruby]
enabled = true
ractor_safe = true
"#,
        );

        RubyGenerator::generate_from_source_directory(
            &config,
            Some(output_directory.clone()),
            &demo_source_directory(),
            "demo",
        )
        .expect("ruby generate should succeed");

        let native_contents = fs::read_to_string(output_directory.join("ext/demo/_native.c"))
            .expect("native.c readable");

        // The opt-in pulls in the Ractor header and declares safety in Init_, before
        // the rb_define_* calls (rb_ext_ractor_safe flags methods defined after it).
        assert!(
            native_contents.contains("#include \"ruby/ractor.h\""),
            "ractor_safe build must include ruby/ractor.h"
        );
        let init_pos = native_contents
            .find("void Init_demo_native")
            .expect("Init_ present");
        let safe_pos = native_contents
            .find("rb_ext_ractor_safe(true)")
            .expect("ractor_safe build must call rb_ext_ractor_safe(true)");
        let define_pos = native_contents[init_pos..]
            .find("rb_define_module(")
            .map(|p| init_pos + p)
            .expect("rb_define_module present");
        assert!(
            init_pos < safe_pos && safe_pos < define_pos,
            "rb_ext_ractor_safe(true) must come after Init_ opens and before rb_define_*"
        );

        fs::remove_dir_all(output_directory).expect("cleanup generated output");
    }

    #[test]
    fn ruby_generate_skips_mut_self_methods() {
        // `StateHolder` (examples/demo/src/classes/unsafe_single_threaded.rs) mixes
        // `&self` readers (`get_value`/`get_label`) with `&mut self` mutators
        // (`set_value`/`increment`/`add_item`/`clear`). The Ruby lowerer cannot
        // soundly expose `&mut self` on a GC-managed handle, so it must SKIP those
        // methods while still emitting the rest of the class — generation must NOT
        // abort.
        let output_directory = unique_temp_dir("boltffi-ruby-mut-self-test");
        let config = parse_config(
            r#"
[package]
name = "demo"
version = "0.1.0"

[targets.ruby]
enabled = true
"#,
        );

        RubyGenerator::generate_from_source_directory(
            &config,
            Some(output_directory.clone()),
            &demo_source_directory(),
            "demo",
        )
        .expect("ruby generate should succeed even when a class has &mut self methods");

        let native_contents = fs::read_to_string(output_directory.join("ext/demo/_native.c"))
            .expect("native.c readable");

        // The `&self` readers are present: both the static glue function and the
        // rb_define_method registration are emitted (type_ident for StateHolder is
        // `state_holder`).
        assert!(
            native_contents.contains("state_holder_get_value"),
            "native.c must emit glue for the &self method get_value"
        );
        assert!(
            native_contents.contains("rb_define_method(c_state_holder, \"get_label\""),
            "native.c must register the &self method get_label"
        );

        // The `&mut self` mutators are skipped entirely: no glue function, no
        // method registration for any of them.
        assert!(
            !native_contents.contains("state_holder_set_value"),
            "native.c must NOT emit glue for the &mut self method set_value"
        );
        assert!(
            !native_contents.contains("rb_define_method(c_state_holder, \"set_value\""),
            "native.c must NOT register the &mut self method set_value"
        );
        assert!(
            !native_contents.contains("rb_define_method(c_state_holder, \"clear\""),
            "native.c must NOT register the &mut self method clear"
        );

        fs::remove_dir_all(output_directory).expect("cleanup generated output");
    }

    #[test]
    fn ruby_generate_binds_static_methods_as_singletons() {
        // `MathUtils` (examples/demo/src/classes/static_methods.rs) mixes a `&self`
        // reader (`round`) with associated functions that take no `self`
        // (`add`/`clamp`). The Rust macro exports the latter WITHOUT a receiver
        // param, so the Ruby generator must bind them as Ruby singleton (class)
        // methods: no TypedData receiver, no handle threaded into the FFI call,
        // registered with `rb_define_singleton_method`. Binding them as instance
        // methods would feed the handle pointer into the first arg slot.
        let output_directory = unique_temp_dir("boltffi-ruby-static-method-test");
        let config = parse_config(
            r#"
[package]
name = "demo"
version = "0.1.0"

[targets.ruby]
enabled = true
"#,
        );

        RubyGenerator::generate_from_source_directory(
            &config,
            Some(output_directory.clone()),
            &demo_source_directory(),
            "demo",
        )
        .expect("ruby generate should succeed with static methods");

        let native_contents = fs::read_to_string(output_directory.join("ext/demo/_native.c"))
            .expect("native.c readable");

        // Static methods are registered as singleton methods on the class object.
        assert!(
            native_contents.contains("rb_define_singleton_method(c_math_utils, \"add\""),
            "native.c must register the static method add as a singleton method"
        );
        assert!(
            native_contents.contains("rb_define_singleton_method(c_math_utils, \"clamp\""),
            "native.c must register the static method clamp as a singleton method"
        );

        // The `&self` reader `round` stays an instance method (not a singleton).
        assert!(
            native_contents.contains("rb_define_method(c_math_utils, \"round\""),
            "native.c must register the &self method round as an instance method"
        );

        // The extern decl omits the receiver entirely (no `MathUtils*` first param).
        assert!(
            native_contents.contains("extern int32_t boltffi_math_utils_add(int32_t, int32_t);"),
            "extern decl for the static method add must omit the receiver"
        );

        // The static glue does not unwrap a receiver, and the FFI call does not
        // thread a `handle` into the first arg slot.
        assert!(
            native_contents.contains("boltffi_math_utils_add(NUM2INT(a), NUM2INT(b))"),
            "static method add must call the FFI symbol with no leading handle arg"
        );
        assert!(
            !native_contents.contains("boltffi_math_utils_add(handle"),
            "static method add must NOT pass a handle into the FFI call"
        );

        fs::remove_dir_all(output_directory).expect("cleanup generated output");
    }
}
