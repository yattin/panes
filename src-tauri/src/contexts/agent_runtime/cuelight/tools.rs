mod application;
mod domain;
mod infrastructure;
mod interfaces;

pub use application::execute_cuelight_tool;
pub use domain::CueLightThreadContext;
pub use infrastructure::auth::set_global_auth_token;
pub use interfaces::prompt::build_cuelight_system_prompt_appendix;
pub use interfaces::tool_specs::{build_cuelight_tool_specs, is_cuelight_tool_name};

#[cfg(test)]
pub(crate) use interfaces::tool_specs::build_cuelight_tool_definitions;

#[cfg(test)]
pub(crate) use application::{
    build_original_script_manifest, get_json_response, original_script_output_dir,
    resolve_existing_file_within_root, OriginalScriptManifestInput,
};

#[cfg(test)]
mod tests;
