/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::sync::Arc;

use common::ConsoleLogger;
use common::FeatureFlags;
use common::SourceLocationKey;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::criterion_group;
use criterion::criterion_main;
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

/// Generate synthetic GraphQL operations for benchmarking
fn generate_operations(count: usize) -> String {
    let mut result = String::with_capacity(count * 200);

    for i in 0..count {
        result.push_str(&format!(
            r#"
query BenchmarkQuery{i}($id: ID!) {{
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
"#,
            i = i
        ));
    }

    result
}

/// Generate synthetic GraphQL fragments for benchmarking
fn generate_fragments(count: usize) -> String {
    let mut result = String::with_capacity(count * 150);

    for i in 0..count {
        result.push_str(&format!(
            r#"
fragment BenchmarkFragment{i} on User {{
  id
  name
  firstName
  lastName
  profilePicture(size: 100) {{
    uri
    width
    height
  }}
}}
"#,
            i = i
        ));
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

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");

    for size in [10, 100, 500].iter() {
        let operations = generate_operations(*size);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("operations", size), &operations, |b, ops| {
            b.iter(|| {
                let source_location = SourceLocationKey::standalone("benchmark.graphql");
                parse_executable(ops, source_location).unwrap()
            });
        });
    }

    for size in [10, 100, 500].iter() {
        let fragments = generate_fragments(*size);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("fragments", size), &fragments, |b, frags| {
            b.iter(|| {
                let source_location = SourceLocationKey::standalone("benchmark.graphql");
                parse_executable(frags, source_location).unwrap()
            });
        });
    }

    group.finish();
}

fn bench_ir_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_build");
    let schema = get_test_schema();

    for size in [10, 100, 500].iter() {
        let operations = generate_operations(*size);
        let source_location = SourceLocationKey::standalone("benchmark.graphql");
        let ast = parse_executable(&operations, source_location).unwrap();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("operations", size), &ast, |b, ast| {
            b.iter(|| {
                build_ir_with_extra_features(
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
                .unwrap()
            });
        });
    }

    group.finish();
}

fn bench_transforms(c: &mut Criterion) {
    let mut group = c.benchmark_group("transforms");
    let schema = get_test_schema();
    let project_config = create_project_config();

    for size in [10, 50, 100].iter() {
        let operations = generate_operations(*size);
        let source_location = SourceLocationKey::standalone("benchmark.graphql");
        let ast = parse_executable(&operations, source_location).unwrap();
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
        .unwrap();
        let program = Arc::new(Program::from_definitions(Arc::clone(&schema), ir));

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("full_pipeline", size),
            &program,
            |b, program| {
                b.iter(|| {
                    apply_transforms(
                        &project_config,
                        Arc::clone(program),
                        Default::default(),
                        Arc::new(ConsoleLogger),
                        None,
                        None,
                        vec![],
                    )
                    .unwrap()
                });
            },
        );
    }

    group.finish();
}

fn bench_full_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_compile");
    let schema = get_test_schema();
    let project_config = create_project_config();

    for size in [10, 50, 100].iter() {
        let operations = generate_operations(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("end_to_end", size),
            &operations,
            |b, ops| {
                b.iter(|| {
                    let source_location = SourceLocationKey::standalone("benchmark.graphql");
                    let ast = parse_executable(ops, source_location).unwrap();
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
                    .unwrap();
                    let program = Arc::new(Program::from_definitions(Arc::clone(&schema), ir));

                    apply_transforms(
                        &project_config,
                        program,
                        Default::default(),
                        Arc::new(ConsoleLogger),
                        None,
                        None,
                        vec![],
                    )
                    .unwrap()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_ir_build,
    bench_transforms,
    bench_full_compile,
);
criterion_main!(benches);
