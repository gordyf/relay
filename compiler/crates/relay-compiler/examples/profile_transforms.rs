/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Profiling binary for the Relay compiler transform pipeline.
//!
//! Run with: cargo run --example profile_transforms --release

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use common::FeatureFlags;
use common::PerfLogEvent;
use common::PerfLogger;
use common::SourceLocationKey;
use graphql_ir::BuilderOptions;
use graphql_ir::FragmentVariablesSemantic;
use graphql_ir::Program;
use graphql_ir::RelayMode;
use graphql_ir::build_ir_with_extra_features;
use graphql_syntax::parse_executable;
use relay_codegen::JsModuleFormat;
use relay_compiler::ProjectConfig;
use relay_config::ProjectName;
use relay_test_schema::get_test_schema;
use relay_transforms::apply_transforms;

/// A perf logger that collects timing data for analysis
struct ProfilingLogger {
    timings: Arc<Mutex<HashMap<String, Vec<Duration>>>>,
}

impl ProfilingLogger {
    fn new() -> Self {
        Self {
            timings: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn report(&self) {
        let timings = self.timings.lock().unwrap();

        // Calculate totals and sort by total time
        let mut with_totals: Vec<_> = timings
            .iter()
            .map(|(name, durations)| {
                let total: Duration = durations.iter().sum();
                let avg = total / durations.len() as u32;
                (name.clone(), total, avg, durations.len())
            })
            .collect();
        with_totals.sort_by(|a, b| b.1.cmp(&a.1));

        println!("\nPer-Transform Timing (sorted by total time):");
        println!("{:-<80}", "");
        println!("{:<45} {:>10} {:>10} {:>8}", "Transform", "Total", "Avg", "Calls");
        println!("{:-<80}", "");

        for (name, total, avg, count) in with_totals.iter().take(25) {
            println!(
                "{:<45} {:>10.2?} {:>10.2?} {:>8}",
                name.split("::").last().unwrap_or(name.as_str()),
                total,
                avg,
                count
            );
        }
        println!("{:-<80}", "");
    }
}

struct ProfilingLogEvent {
    event_name: &'static str,
    timings: Arc<Mutex<HashMap<String, Vec<Duration>>>>,
}

impl PerfLogEvent for ProfilingLogEvent {
    type Timer = (&'static str, Instant);

    fn number(&self, _name: &'static str, _number: usize) {}
    fn bool(&self, _name: &'static str, _value: bool) {}
    fn string(&self, _name: &'static str, _value: String) {}

    fn start(&self, name: &'static str) -> Self::Timer {
        (name, Instant::now())
    }

    fn stop(&self, timer: Self::Timer) {
        let (name, start) = timer;
        let elapsed = start.elapsed();
        let key = format!("{}::{}", self.event_name, name);
        let mut timings = self.timings.lock().unwrap();
        timings.entry(key).or_default().push(elapsed);
    }

    fn complete(self) {}
}

impl PerfLogger for ProfilingLogger {
    type PerfLogEvent = ProfilingLogEvent;

    fn create_event(&self, name: &'static str) -> Self::PerfLogEvent {
        ProfilingLogEvent {
            event_name: name,
            timings: Arc::clone(&self.timings),
        }
    }
}

/// Generate synthetic GraphQL operations for profiling
fn generate_operations(count: usize) -> String {
    let mut result = String::with_capacity(count * 300);

    for i in 0..count {
        let variant = i % 5;
        match variant {
            0 => {
                result.push_str(&format!(
                    r#"
query ProfileQuery{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on User {{
      name
      firstName
      lastName
      profilePicture(size: 100) {{
        uri
        width
        height
      }}
      friends(first: 10) {{
        edges {{
          node {{
            id
            name
          }}
        }}
      }}
    }}
  }}
}}
"#
                ));
            }
            1 => {
                result.push_str(&format!(
                    r#"
query ProfileQuery{i}($id: ID!, $first: Int!, $after: ID) {{
  node(id: $id) {{
    id
    ... on User {{
      name
      friends(first: $first, after: $after) {{
        pageInfo {{
          hasNextPage
          endCursor
        }}
        edges {{
          cursor
          node {{
            id
            name
            profilePicture(size: 50) {{
              uri
            }}
          }}
        }}
      }}
    }}
  }}
}}
"#
                ));
            }
            2 => {
                result.push_str(&format!(
                    r#"
query ProfileQuery{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on User {{
      name
      birthdate {{ year month day }}
      hometown {{ id name }}
    }}
    ... on Page {{
      name
      likers {{ count }}
    }}
  }}
}}
"#
                ));
            }
            3 => {
                result.push_str(&format!(
                    r#"
query ProfileQuery{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on Comment {{
      body {{ text }}
      author {{
        name
        profilePicture(size: 32) {{ uri }}
      }}
    }}
  }}
}}
"#
                ));
            }
            _ => {
                result.push_str(&format!(
                    r#"
query ProfileQuery{i}($id: ID!) {{
  node(id: $id) {{
    id
    ... on Actor {{ name }}
    ... on Named {{ name }}
  }}
}}
"#
                ));
            }
        }
    }

    result
}

fn create_project_config() -> ProjectConfig {
    ProjectConfig {
        name: ProjectName::default(),
        feature_flags: Arc::new(FeatureFlags::default()),
        js_module_format: JsModuleFormat::Haste,
        ..Default::default()
    }
}

fn main() {
    let iterations = 10;
    let operation_count = 500;

    println!("Profiling Relay Compiler Transforms");
    println!("====================================");
    println!("Operations: {}", operation_count);
    println!("Iterations: {}", iterations);
    println!();

    // Warmup: Load schema
    let schema_start = Instant::now();
    let schema = get_test_schema();
    let schema_time = schema_start.elapsed();
    println!("Schema load time: {:?}", schema_time);

    let project_config = create_project_config();

    // Generate workload
    let gen_start = Instant::now();
    let operations = generate_operations(operation_count);
    let gen_time = gen_start.elapsed();
    println!("Generation time: {:?} ({} bytes)", gen_time, operations.len());

    // Parse
    let parse_start = Instant::now();
    let source_location = SourceLocationKey::standalone("profile.graphql");
    let ast = parse_executable(&operations, source_location).expect("Failed to parse");
    let parse_time = parse_start.elapsed();
    println!("Parse time: {:?} ({} definitions)", parse_time, ast.definitions.len());

    // Build IR
    let ir_start = Instant::now();
    let ir = build_ir_with_extra_features(
        &schema,
        &ast.definitions,
        &BuilderOptions {
            allow_undefined_fragment_spreads: false,
            allow_non_overlapping_abstract_spreads: false,
            fragment_variables_semantic: FragmentVariablesSemantic::PassedValue,
            relay_mode: Some(RelayMode),
            default_anonymous_operation_name: None,
            allow_custom_scalar_literals: true,
        },
    )
    .expect("Failed to build IR");
    let ir_time = ir_start.elapsed();
    println!("IR build time: {:?}", ir_time);

    let program = Arc::new(Program::from_definitions(Arc::clone(&schema), ir));
    println!();

    // Create profiling logger
    let perf_logger = Arc::new(ProfilingLogger::new());

    // Run transform pipeline multiple times
    let mut transform_times = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let start = Instant::now();
        let result = apply_transforms(
            &project_config,
            Arc::clone(&program),
            Default::default(),
            Arc::clone(&perf_logger),
            None,
            None,
            vec![],
        );
        let elapsed = start.elapsed();
        transform_times.push(elapsed);

        if i == 0 {
            match &result {
                Ok(programs) => {
                    println!(
                        "First iteration: {} ops, {} frags in {:?}",
                        programs.normalization.operations().count(),
                        programs.reader.fragments().count(),
                        elapsed
                    );
                }
                Err(e) => {
                    eprintln!("Transform error: {:?}", e);
                    std::process::exit(1);
                }
            }
        }

        let _ = std::hint::black_box(result);
    }

    // Calculate statistics
    transform_times.sort();
    let total: Duration = transform_times.iter().sum();
    let avg = total / iterations as u32;
    let min = transform_times.first().unwrap();
    let max = transform_times.last().unwrap();
    let median = &transform_times[iterations / 2];
    let p95 = &transform_times[(iterations as f64 * 0.95) as usize];

    println!();
    println!("Transform Pipeline Statistics ({} iterations):", iterations);
    println!("  Min:    {:?}", min);
    println!("  Median: {:?}", median);
    println!("  Avg:    {:?}", avg);
    println!("  P95:    {:?}", p95);
    println!("  Max:    {:?}", max);
    println!();
    println!(
        "Throughput: {:.2} ops/sec",
        operation_count as f64 / avg.as_secs_f64()
    );

    // Print detailed timing report
    perf_logger.report();
}
