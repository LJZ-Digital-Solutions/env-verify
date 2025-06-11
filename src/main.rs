use anyhow::{Context, Result, anyhow, bail};
use clap::{Arg, Command};
use jsonschema::ValidationError;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use tracing::{debug, error, info};

//
// Global to reuse cargo.toml metadata
//
const NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
// Argument IDs
const ARG_SCHEMA: &str = "schema";
const ARG_INPUT: &str = "input";
const ARG_ENV_VARS_INPUT: &str = "env-vars";
const ARG_ENV_SECRETS_INPUT: &str = "env-secrets";
const ARG_OUTPUT: &str = "output";

fn main() -> Result<()> {
    let matches = Command::new(NAME)
        .version(VERSION)
        .author(AUTHORS)
        .about(DESCRIPTION)
        .arg(
            Arg::new(ARG_SCHEMA)
                .short('s')
                .long("schema")
                .value_name("FILE")
                .help("Path to JSON schema to validate against")
                .required(true),
        )
        .arg(
            Arg::new(ARG_INPUT)
                .short('i')
                .long("input")
                .value_name("FILE")
                .help("Path to input JSON file that needs to be validated")
                .required(true),
        )
        .arg(
            Arg::new(ARG_ENV_VARS_INPUT)
                .short('e')
                .long("env-vars")
                .value_name("FILE")
                .help(
                    "Path to JSON file that contains all environment variables (key, value) pairs",
                ),
        )
        .arg(
            Arg::new(ARG_ENV_SECRETS_INPUT)
                .short('x')
                .long("env-secrets")
                .value_name("FILE")
                .help("Path to JSON file that contains all environment secrets (key, value) pairs"),
        )
        .arg(
            Arg::new(ARG_OUTPUT)
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Path to write the validated and substituted JSON to"),
        )
        .get_matches();

    // Set up simple logging to stdout
    tracing_subscriber::fmt()
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::uptime()) // Time since start
        .init();

    // Is already done by CLAP, but the type safety is nice
    let schema_path = matches
        .get_one::<String>(ARG_SCHEMA)
        .ok_or_else(|| anyhow!("Schema argument is required"))?;
    let input_path = matches
        .get_one::<String>(ARG_INPUT)
        .ok_or_else(|| anyhow!("Input argument is required"))?;
    let env_vars_path = matches
        .get_one::<String>(ARG_ENV_VARS_INPUT)
        .map(String::as_str);
    let env_secrets_path = matches
        .get_one::<String>(ARG_ENV_SECRETS_INPUT)
        .map(String::as_str);
    let output_path = matches.get_one::<String>(ARG_OUTPUT).map(String::as_str);

    if let Err(e) = run(
        schema_path,
        input_path,
        env_vars_path,
        env_secrets_path,
        output_path,
    ) {
        error!("{}", e);

        // Print the full error chain
        let mut source = e.source();
        while let Some(err) = source {
            eprintln!("       Caused by: {err}");
            source = err.source();
        }
        std::process::exit(1);
    }

    Ok(())
}

// Validate a JSON input against a specific JSON schema
fn validate_json(schema: &Value, input: &Value) -> Result<()> {
    let validator =
        jsonschema::validator_for(schema).with_context(|| "Failed to compile JSON schema")?;

    let errors: Vec<ValidationError> = validator.iter_errors(input).collect();

    if !errors.is_empty() {
        let mut error_msg = format!("Schema validation failed with {} error(s):", errors.len());

        for (i, error) in errors.iter().enumerate() {
            // Then change your code to:
            write!(
                error_msg,
                "\n  {}. Path: '{}' - {}",
                i + 1,
                error.instance_path,
                error
            )
            .unwrap();
        }

        bail!(error_msg);
    }

    Ok(())
}

// Helper function to parse substitutes from an optional path
fn parse_substitutes_from_path(path: Option<&str>) -> Result<Option<HashMap<String, String>>> {
    if let Some(path) = path {
        let content: String =
            fs::read_to_string(path).with_context(|| format!("Failed to read file: {path}"))?;
        let json: Value = serde_json::from_str(&content)
            .with_context(|| format!("Substitutes are not given as valid JSON: {path}"))?;

        // Check if it's an object and get key count
        if let Some(obj) = json.as_object() {
            let key_count = obj.len();

            // Convert to hashmap that can be used for quick lookups
            let mut map = HashMap::new();
            for (key, value) in obj {
                let string_value = match value {
                    Value::String(s) => s.clone(),
                    _ => value.to_string().trim_matches('"').to_string(),
                };
                map.insert(key.to_lowercase(), string_value);
            }
            info!("Loaded {} substitutes from {}", key_count, path);
            Ok(Some(map))
        } else {
            bail!(
                "Substitutes file must contain a JSON object, not {}",
                match json {
                    Value::Array(_) => "an array",
                    Value::String(_) => "a string",
                    Value::Number(_) => "a number",
                    Value::Bool(_) => "a boolean",
                    Value::Null => "null",
                    Value::Object(_) => "an object",
                }
            );
        }
    } else {
        info!("No substitutes were specified");
        Ok(None)
    }
}

fn substitute_values(
    input: &mut Value,
    env_secrets: Option<&HashMap<String, String>>,
    env_vars: Option<&HashMap<String, String>>,
) -> Result<()> {
    let template_regex =
        Regex::new(r"\{\{\s*([^}]+)\s*\}\}").context("Failed to compile template regex")?;

    substitute_recursive(input, env_secrets, env_vars, &template_regex, "$")?;
    Ok(())
}

fn substitute_recursive(
    value: &mut Value,
    env_secrets: Option<&HashMap<String, String>>,
    env_vars: Option<&HashMap<String, String>>,
    regex: &Regex,
    json_path: &str,
) -> Result<()> {
    match value {
        Value::String(s) => {
            let original = s.clone();
            *s = substitute_string(s, env_secrets, env_vars, regex, json_path)?;

            // Log if substitution occurred
            if *s != original {
                debug!(
                    "Substituted value at path '{}': '{}' -> '{}'",
                    json_path, original, s
                );
            }
        }
        Value::Object(obj) => {
            for (key, v) in obj.iter_mut() {
                let new_path = if json_path == "$" {
                    format!("$.{key}")
                } else {
                    format!("{json_path}.{key}")
                };
                substitute_recursive(v, env_secrets, env_vars, regex, &new_path)?;
            }
        }
        Value::Array(arr) => {
            for (index, item) in arr.iter_mut().enumerate() {
                let new_path = format!("{json_path}[{index}]");
                substitute_recursive(item, env_secrets, env_vars, regex, &new_path)?;
            }
        }
        _ => {} // Numbers, booleans, null don't need substitution
    }
    Ok(())
}

fn substitute_string(
    s: &str,
    env_secrets: Option<&HashMap<String, String>>,
    env_vars: Option<&HashMap<String, String>>,
    regex: &Regex,
    json_path: &str,
) -> Result<String> {
    let mut result = s.to_string();

    for cap in regex.captures_iter(s) {
        let full_match = &cap[0]; // The entire {{ NAME }} part
        let var_name = cap[1].trim();
        let var_name_lower = var_name.to_lowercase();

        // Try env_secrets first, then env_vars
        let (replacement, source) = if let Some(secrets) = env_secrets {
            if let Some(value) = secrets.get(&var_name_lower) {
                (value.clone(), "env_secrets")
            } else if let Some(vars) = env_vars {
                if let Some(value) = vars.get(&var_name_lower) {
                    (value.clone(), "env_vars")
                } else {
                    bail!(
                        "Substitution variable '{}' specified at path '{}', but its value was not found in env_secrets or env_vars",
                        var_name,
                        json_path
                    );
                }
            } else {
                bail!(
                    "Substitution variable '{}' specified at path '{}', but its value was not found in env_secrets, and no env_vars was specified",
                    var_name,
                    json_path
                );
            }
        } else if let Some(vars) = env_vars {
            if let Some(value) = vars.get(&var_name_lower) {
                (value.clone(), "env_vars")
            } else {
                bail!(
                    "Substitution variable '{}' specified at path '{}', but its value was not found in env_vars, and no env_secrets was specified",
                    var_name,
                    json_path
                );
            }
        } else {
            bail!(
                "Substitution variable '{}' specified at path '{}', but no substitution sources (env_vars, env_secrets) were provided",
                var_name,
                json_path
            );
        };

        result = result.replace(full_match, &replacement);
        debug!(
            "Replaced '{{{{ {} }}}}' with value from {} at JSON path '{}'",
            var_name, source, json_path
        );
    }

    Ok(result)
}

fn run(
    schema_path: &str,
    input_path: &str,
    env_vars_path: Option<&str>,
    env_secrets_path: Option<&str>,
    output_path: Option<&str>,
) -> Result<()> {
    // Read actual files
    let schema = fs::read_to_string(schema_path)
        .with_context(|| format!("Failed to read schema file: {schema_path}"))?;
    let input = fs::read_to_string(input_path)
        .with_context(|| format!("Failed to read input file: {input_path}"))?;

    // Error if the output path already exists
    if let Some(output_path) = output_path {
        if fs::metadata(output_path).is_ok() {
            bail!("Output file '{output_path}' already exists. Will not overwrite");
        }
    }

    // Substitutes can be used to produce the final JSON output later (this is the JSON that gets validated)
    info!("Parsing environment variable substitutes");
    let env_vars = parse_substitutes_from_path(env_vars_path)?;
    info!("Parsing environment secret substitutes");
    let env_secrets = parse_substitutes_from_path(env_secrets_path)?;

    // Convert to JSON
    let schema: Value = serde_json::from_str(&schema)
        .with_context(|| format!("Schema file is not valid JSON: {schema_path}"))?;
    let mut input: Value = serde_json::from_str(&input)
        .with_context(|| format!("Input file is not valid JSON: {input_path}"))?;

    // Convert &option<hashmasp> to option<&hashmap>
    let env_vars = env_vars.as_ref().map(|m| m as &HashMap<String, String>);
    let env_secrets = env_secrets.as_ref().map(|m| m as &HashMap<String, String>);

    info!("Scanning for substitution placeholders");
    substitute_values(&mut input, env_secrets, env_vars)?;
    info!("Substitutions succeeded, performing schema validation");
    validate_json(&schema, &input)?;
    info!("Validation successful");

    // Write to output file if specified
    if let Some(output_path) = output_path {
        info!("Writing validated JSON to output file: {output_path}");

        let pretty_json =
            serde_json::to_string_pretty(&input).context("Failed to serialize JSON for output")?;

        let len = pretty_json.len();
        fs::write(output_path, pretty_json)
            .with_context(|| format!("Failed to write output file: {output_path}"))?;

        info!(
            "Successfully wrote output JSON ({} bytes) to {}",
            len, output_path
        );
    } else {
        info!("No output file specified, done");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[derive(Debug)]
    struct TestCase {
        name: String,
        input_path: PathBuf,
        schema_path: PathBuf,
        expected_output_path: PathBuf,
        env_vars_path: Option<PathBuf>,
        env_secrets_path: Option<PathBuf>,
    }

    impl TestCase {
        fn from_directory(dir: &Path) -> Result<Self> {
            let name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow!("Invalid directory name: {:?}", dir))?
                .to_string();

            // Check for required files
            let input_path = dir.join("input.json");
            let schema_path = dir.join("schema.json");
            let expected_output_path = dir.join("expected-output.json");

            if !input_path.exists() {
                bail!("Missing required file in test '{}': input.json", name);
            }
            if !schema_path.exists() {
                bail!("Missing required file in test '{}': schema.json", name);
            }
            if !expected_output_path.exists() {
                bail!(
                    "Missing required file in test '{}': expected-output.json",
                    name
                );
            }

            // Check for optional files
            let env_vars_path = dir.join("env-vars.json");
            let env_secrets_path = dir.join("env-secrets.json");

            Ok(TestCase {
                name,
                input_path,
                schema_path,
                expected_output_path,
                env_vars_path: if env_vars_path.exists() {
                    Some(env_vars_path)
                } else {
                    None
                },
                env_secrets_path: if env_secrets_path.exists() {
                    Some(env_secrets_path)
                } else {
                    None
                },
            })
        }
    }

    fn discover_test_cases() -> Result<Vec<TestCase>> {
        let tests_dir = Path::new("tests");

        if !tests_dir.exists() {
            bail!("Tests directory does not exist: {}", tests_dir.display());
        }

        let mut test_cases = Vec::new();

        for entry in fs::read_dir(tests_dir)
            .with_context(|| format!("Failed to read tests directory: {}", tests_dir.display()))?
        {
            let entry = entry.with_context(|| "Failed to read directory entry")?;
            let path = entry.path();

            if path.is_dir() {
                // This will now error instead of skipping invalid directories
                let test_case = TestCase::from_directory(&path)
                    .with_context(|| format!("Invalid test directory: {}", path.display()))?;
                test_cases.push(test_case);
            }
        }

        if test_cases.is_empty() {
            bail!("No valid test cases found in tests directory");
        }

        // Sort by name for consistent test order
        test_cases.sort_by(|a, b| a.name.cmp(&b.name));

        println!(
            "Discovered {} test case(s): {}",
            test_cases.len(),
            test_cases
                .iter()
                .map(|tc| tc.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        Ok(test_cases)
    }

    fn run_test_case(test_case: &TestCase) -> Result<()> {
        println!("Running test case: {}", test_case.name);

        // Create a temporary directory for output
        let temp_dir = TempDir::new()
            .with_context(|| format!("Failed to create temp dir for test '{}'", test_case.name))?;
        let actual_output_path = temp_dir.path().join("actual_output.json");

        // Run the function
        run(
            test_case.schema_path.to_str().unwrap(),
            test_case.input_path.to_str().unwrap(),
            test_case
                .env_vars_path
                .as_ref()
                .map(|p| p.to_str().unwrap()),
            test_case
                .env_secrets_path
                .as_ref()
                .map(|p| p.to_str().unwrap()),
            Some(actual_output_path.to_str().unwrap()),
        )
        .with_context(|| format!("Test case '{}' failed during execution", test_case.name))?;

        // Read and parse the expected output
        let expected_content =
            fs::read_to_string(&test_case.expected_output_path).with_context(|| {
                format!(
                    "Failed to read expected output for test '{}'",
                    test_case.name
                )
            })?;
        let expected_json: Value = serde_json::from_str(&expected_content).with_context(|| {
            format!(
                "Expected output is not valid JSON for test '{}'",
                test_case.name
            )
        })?;

        // Read and parse the actual output
        let actual_content = fs::read_to_string(&actual_output_path).with_context(|| {
            format!("Failed to read actual output for test '{}'", test_case.name)
        })?;
        let actual_json: Value = serde_json::from_str(&actual_content).with_context(|| {
            format!(
                "Actual output is not valid JSON for test '{}'",
                test_case.name
            )
        })?;

        // Compare JSON values
        if expected_json != actual_json {
            bail!(
                "Test case '{}' failed: JSON output mismatch\nExpected:\n{}\nActual:\n{}",
                test_case.name,
                serde_json::to_string_pretty(&expected_json)?,
                serde_json::to_string_pretty(&actual_json)?
            );
        }

        println!("✓ Test case '{}' passed", test_case.name);
        Ok(())
    }

    #[test]
    fn test_all_cases() -> Result<()> {
        // Initialize tracing for tests (optional, you might want to disable logging in tests)
        let _ = tracing_subscriber::fmt()
            .with_target(false)
            .without_time()
            .with_max_level(tracing::Level::WARN) // Reduce noise in tests
            .try_init();

        let test_cases = discover_test_cases().context("Failed to discover test cases")?;

        let mut failures = Vec::new();

        for test_case in &test_cases {
            if let Err(e) = run_test_case(test_case) {
                failures.push((test_case.name.clone(), e));
            }
        }

        if !failures.is_empty() {
            let mut error_msg = format!("{} test case(s) failed:\n", failures.len());
            for (name, error) in failures {
                writeln!(
                    error_msg,
                    "  - {}: {}",
                    name,
                    error.to_string().replace('\n', "\n    ")
                )
                .unwrap();
            }
            bail!(error_msg);
        }

        println!("All {} test case(s) passed!", test_cases.len());
        Ok(())
    }

    #[test]
    fn test_individual_cases() -> Result<()> {
        // This creates individual test functions for each case,
        // making it easier to run specific tests
        let _ = tracing_subscriber::fmt()
            .with_target(false)
            .without_time()
            .with_max_level(tracing::Level::WARN)
            .try_init();

        let test_cases = discover_test_cases().context("Failed to discover test cases")?;

        // Run each test case individually so failures are isolated
        for test_case in test_cases {
            run_test_case(&test_case)
                .with_context(|| format!("Individual test failed: {}", test_case.name))?;
        }

        Ok(())
    }
}
