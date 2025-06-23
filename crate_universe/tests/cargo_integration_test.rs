//! cargo_bazel integration tests that run Cargo to test generating metadata.

extern crate cargo_bazel;
extern crate serde_json;
extern crate tempfile;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{ensure, Context, Result};
use cargo_bazel::cli::{splice, SpliceOptions};
use serde_json::json;

fn should_skip_test() -> bool {
    // All test cases require network access to build pull crate metadata
    // so that we can actually run `cargo tree`. However, RBE (and perhaps
    // other environments) disallow or don't support this. In those cases,
    // we just skip this test case.
    use std::net::ToSocketAddrs;
    if "github.com:443".to_socket_addrs().is_err() {
        eprintln!("This test case requires network access.");
        true
    } else {
        false
    }
}

fn setup_cargo_env(rfiles: &runfiles::Runfiles) -> Result<(PathBuf, PathBuf)> {
    let cargo = runfiles::rlocation!(
        rfiles,
        env::var("CARGO").context("CARGO environment variable must be set.")?
    )
    .unwrap();
    let rustc = runfiles::rlocation!(
        rfiles,
        env::var("RUSTC").context("RUSTC environment variable must be set.")?
    )
    .unwrap();
    ensure!(cargo.exists());
    ensure!(rustc.exists());
    // If $RUSTC is a relative path it can cause issues with
    // `cargo_metadata::MetadataCommand`. Just to be on the safe side, we make
    // both of these env variables absolute paths.
    if cargo != PathBuf::from(env::var("CARGO").unwrap()) {
        env::set_var("CARGO", cargo.as_os_str());
    }
    if rustc != PathBuf::from(env::var("RUSTC").unwrap()) {
        env::set_var("RUSTC", rustc.as_os_str());
    }

    let cargo_home = PathBuf::from(
        env::var("TEST_TMPDIR").context("TEST_TMPDIR environment variable must be set.")?,
    )
    .join("cargo_home");
    env::set_var("CARGO_HOME", cargo_home.as_os_str());
    fs::create_dir_all(&cargo_home)?;

    println!("Environment:");
    println!("\tRUSTC={}", rustc.display());
    println!("\tCARGO={}", cargo.display());
    println!("\tCARGO_HOME={}", cargo_home.display());

    Ok((cargo, rustc))
}

fn run(
    repository_name: &str,
    manifests: HashMap<String, String>,
    lockfile: &str,
) -> cargo_metadata::Metadata {
    let scratch = tempfile::tempdir().unwrap();
    let runfiles = runfiles::Runfiles::create().unwrap();

    let (cargo, rustc) = setup_cargo_env(&runfiles).unwrap();

    let splicing_manifest = scratch.path().join("splicing_manifest.json");
    fs::write(
        &splicing_manifest,
        serde_json::to_string(&json!({
            "manifests": manifests,
            "direct_packages": {},
            "resolver_version": "2"
        }))
        .unwrap(),
    )
    .unwrap();

    let config = scratch.path().join("config.json");
    fs::write(
        &config,
        serde_json::to_string(&json!({
            "generate_binaries": false,
            "generate_build_scripts": false,
            "rendering": {
                "generate_cargo_toml_env_vars": true,
                "repository_name": repository_name,
                "regen_command": "//crate_universe:cargo_integration_test"
            },
            "supported_platform_triples": [
                "wasm32-unknown-unknown",
                "x86_64-apple-darwin",
                "x86_64-pc-windows-msvc",
                "x86_64-unknown-linux-gnu",
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    splice(SpliceOptions {
        splicing_manifest,
        cargo_lockfile: Some(runfiles::rlocation!(runfiles, lockfile).unwrap()),
        repin: None,
        workspace_dir: None,
        output_dir: scratch.path().join("out"),
        dry_run: false,
        cargo_config: None,
        config,
        cargo,
        rustc,
        repository_name: String::from("crates_index"),
    })
    .unwrap();

    let metadata = serde_json::from_str::<cargo_metadata::Metadata>(
        &fs::read_to_string(scratch.path().join("out").join("metadata.json")).unwrap(),
    )
    .unwrap();

    metadata
}

// See crate_universe/test_data/metadata/target_features/Cargo.toml for input.
#[test]
fn feature_generator() {
    if should_skip_test() {
        eprintln!("Skipping!");
        return;
    }

    let r = runfiles::Runfiles::create().unwrap();
    let metadata = run(
        "target_feature_test",
        HashMap::from([(
            runfiles::rlocation!(
                r,
                "rules_rust/crate_universe/test_data/metadata/target_features/Cargo.toml"
            )
            .unwrap()
            .to_string_lossy()
            .to_string(),
            "//:test_input".to_string(),
        )]),
        "rules_rust/crate_universe/test_data/metadata/target_features/Cargo.lock",
    );

    assert_eq!(
        json!({
            "common": {},
            "selects": {
              "x86_64-apple-darwin": {
                "deps": [
                  {
                    "features": [
                      "default"
                    ],
                    "id": "arrayvec 0.7.2",
                    "target_name": "arrayvec"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "bitflags 1.3.2",
                    "target_name": "bitflags"
                  },
                  {
                    "features": [],
                    "id": "block 0.1.6",
                    "optional": true,
                    "target_name": "block"
                  },
                  {
                    "features": [],
                    "id": "core-graphics-types 0.1.1",
                    "target_name": "core_graphics_types"
                  },
                  {
                    "features": [],
                    "id": "foreign-types 0.3.2",
                    "optional": true,
                    "target_name": "foreign_types"
                  },
                  {
                    "features": [],
                    "id": "fxhash 0.2.1",
                    "target_name": "fxhash"
                  },
                  {
                    "features": [],
                    "id": "log 0.4.17",
                    "target_name": "log"
                  },
                  {
                    "alias": "mtl",
                    "features": [
                      "default"
                    ],
                    "id": "metal 0.24.0",
                    "target_name": "metal"
                  },
                  {
                    "features": [
                      "clone",
                      "default",
                      "msl-out"
                    ],
                    "id": "naga 0.10.0",
                    "target_name": "naga"
                  },
                  {
                    "features": [],
                    "id": "objc 0.2.7",
                    "target_name": "objc"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "parking_lot 0.12.1",
                    "target_name": "parking_lot"
                  },
                  {
                    "features": [],
                    "id": "profiling 1.0.7",
                    "target_name": "profiling"
                  },
                  {
                    "features": [],
                    "id": "raw-window-handle 0.5.0",
                    "target_name": "raw_window_handle"
                  },
                  {
                    "features": [],
                    "id": "thiserror 1.0.37",
                    "target_name": "thiserror"
                  },
                  {
                    "alias": "wgt",
                    "features": [],
                    "id": "wgpu-types 0.14.1",
                    "target_name": "wgpu_types"
                  }
                ],
                "features": [
                  "block",
                  "default",
                  "foreign-types",
                  "metal"
                ]
              },
              "x86_64-pc-windows-msvc": {
                "deps": [
                  {
                    "features": [
                      "default"
                    ],
                    "id": "arrayvec 0.7.2",
                    "target_name": "arrayvec"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "ash 0.37.1+1.3.235",
                    "optional": true,
                    "target_name": "ash"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "bit-set 0.5.3",
                    "optional": true,
                    "target_name": "bit_set"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "bitflags 1.3.2",
                    "target_name": "bitflags"
                  },
                  {
                    "alias": "native",
                    "features": [
                      "libloading"
                    ],
                    "id": "d3d12 0.5.0",
                    "optional": true,
                    "target_name": "d3d12"
                  },
                  {
                    "features": [],
                    "id": "fxhash 0.2.1",
                    "target_name": "fxhash"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "gpu-alloc 0.5.3",
                    "optional": true,
                    "target_name": "gpu_alloc"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "gpu-descriptor 0.2.3",
                    "optional": true,
                    "target_name": "gpu_descriptor"
                  },
                  {
                    "features": [],
                    "id": "libloading 0.7.4",
                    "optional": true,
                    "target_name": "libloading"
                  },
                  {
                    "features": [],
                    "id": "log 0.4.17",
                    "target_name": "log"
                  },
                  {
                    "features": [
                      "clone",
                      "default",
                      "hlsl-out",
                      "spv-out"
                    ],
                    "id": "naga 0.10.0",
                    "target_name": "naga"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "parking_lot 0.12.1",
                    "target_name": "parking_lot"
                  },
                  {
                    "features": [],
                    "id": "profiling 1.0.7",
                    "target_name": "profiling"
                  },
                  {
                    "features": [],
                    "id": "range-alloc 0.1.2",
                    "optional": true,
                    "target_name": "range_alloc"
                  },
                  {
                    "features": [],
                    "id": "raw-window-handle 0.5.0",
                    "target_name": "raw_window_handle"
                  },
                  {
                    "features": [],
                    "id": "renderdoc-sys 0.7.1",
                    "optional": true,
                    "target_name": "renderdoc_sys"
                  },
                  {
                    "features": [
                      "union"
                    ],
                    "id": "smallvec 1.10.0",
                    "optional": true,
                    "target_name": "smallvec"
                  },
                  {
                    "features": [],
                    "id": "thiserror 1.0.37",
                    "target_name": "thiserror"
                  },
                  {
                    "alias": "wgt",
                    "features": [],
                    "id": "wgpu-types 0.14.1",
                    "target_name": "wgpu_types"
                  },
                  {
                    "features": [
                      "d3d11",
                      "d3d11_1",
                      "d3d11_2",
                      "d3d11sdklayers",
                      "d3d12",
                      "d3d12sdklayers",
                      "d3d12shader",
                      "dcomp",
                      "dxgi1_6",
                      "libloaderapi",
                      "windef",
                      "winuser"
                    ],
                    "id": "winapi 0.3.9",
                    "target_name": "winapi"
                  }
                ],
                "features": [
                  "ash",
                  "bit-set",
                  "default",
                  "dx11",
                  "dx12",
                  "gpu-alloc",
                  "gpu-descriptor",
                  "libloading",
                  "native",
                  "range-alloc",
                  "renderdoc",
                  "renderdoc-sys",
                  "smallvec",
                  "vulkan"
                ]
              },
              "x86_64-unknown-linux-gnu": {
                "deps": [
                  {
                    "features": [
                      "default"
                    ],
                    "id": "arrayvec 0.7.2",
                    "target_name": "arrayvec"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "ash 0.37.1+1.3.235",
                    "optional": true,
                    "target_name": "ash"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "bitflags 1.3.2",
                    "target_name": "bitflags"
                  },
                  {
                    "features": [],
                    "id": "fxhash 0.2.1",
                    "target_name": "fxhash"
                  },
                  {
                    "features": [],
                    "id": "glow 0.11.2",
                    "optional": true,
                    "target_name": "glow"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "gpu-alloc 0.5.3",
                    "optional": true,
                    "target_name": "gpu_alloc"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "gpu-descriptor 0.2.3",
                    "optional": true,
                    "target_name": "gpu_descriptor"
                  },
                  {
                    "alias": "egl",
                    "features": [
                      "default",
                      "dynamic"
                    ],
                    "id": "khronos-egl 4.1.0",
                    "optional": true,
                    "target_name": "khronos_egl"
                  },
                  {
                    "features": [],
                    "id": "libloading 0.7.4",
                    "optional": true,
                    "target_name": "libloading"
                  },
                  {
                    "features": [],
                    "id": "log 0.4.17",
                    "target_name": "log"
                  },
                  {
                    "features": [
                      "clone",
                      "default",
                      "glsl-out",
                      "spv-out"
                    ],
                    "id": "naga 0.10.0",
                    "target_name": "naga"
                  },
                  {
                    "features": [
                      "default"
                    ],
                    "id": "parking_lot 0.12.1",
                    "target_name": "parking_lot"
                  },
                  {
                    "features": [],
                    "id": "profiling 1.0.7",
                    "target_name": "profiling"
                  },
                  {
                    "features": [],
                    "id": "raw-window-handle 0.5.0",
                    "target_name": "raw_window_handle"
                  },
                  {
                    "features": [],
                    "id": "renderdoc-sys 0.7.1",
                    "optional": true,
                    "target_name": "renderdoc_sys"
                  },
                  {
                    "features": [
                      "union"
                    ],
                    "id": "smallvec 1.10.0",
                    "optional": true,
                    "target_name": "smallvec"
                  },
                  {
                    "features": [],
                    "id": "thiserror 1.0.37",
                    "target_name": "thiserror"
                  },
                  {
                    "alias": "wgt",
                    "features": [],
                    "id": "wgpu-types 0.14.1",
                    "target_name": "wgpu_types"
                  }
                ],
                "features": [
                  "ash",
                  "default",
                  "egl",
                  "gles",
                  "glow",
                  "gpu-alloc",
                  "gpu-descriptor",
                  "libloading",
                  "renderdoc",
                  "renderdoc-sys",
                  "smallvec",
                  "vulkan"
                ]
              }
            }
        }),
        metadata.workspace_metadata["cargo-bazel"]["resolver_metadata"]["wgpu-hal 0.14.1"],
    );
}

// See crate_universe/test_data/metadata/target_cfg_features/Cargo.toml for input.
#[test]
fn feature_generator_cfg_features() {
    if should_skip_test() {
        eprintln!("Skipping!");
        return;
    }

    let r = runfiles::Runfiles::create().unwrap();
    let metadata = run(
        "target_cfg_features_test",
        HashMap::from([(
            runfiles::rlocation!(
                r,
                "rules_rust/crate_universe/test_data/metadata/target_cfg_features/Cargo.toml"
            )
            .unwrap()
            .to_string_lossy()
            .to_string(),
            "//:test_input".to_string(),
        )]),
        "rules_rust/crate_universe/test_data/metadata/target_cfg_features/Cargo.lock",
    );

    assert_eq!(
        json!({
            "autocfg 1.1.0": {
              "common": {},
              "selects": {}
            },
            "pin-project-lite 0.2.9": {
              "common": {},
              "selects": {}
            },
            "target_cfg_features 0.1.0": {
              "common": {},
              "selects": {
                "wasm32-unknown-unknown": {
                  "deps": [
                    {
                      "features": [
                        "default"
                      ],
                      "id": "tokio 1.25.0",
                      "target_name": "tokio"
                    }
                  ]
                },
                "x86_64-apple-darwin": {
                  "deps": [
                    {
                      "features": [
                        "default",
                        "fs"
                      ],
                      "id": "tokio 1.25.0",
                      "target_name": "tokio"
                    }
                  ]
                },
                "x86_64-pc-windows-msvc": {
                  "deps": [
                    {
                      "features": [
                        "default"
                      ],
                      "id": "tokio 1.25.0",
                      "target_name": "tokio"
                    }
                  ]
                },
                "x86_64-unknown-linux-gnu": {
                  "deps": [
                    {
                      "features": [
                        "default",
                        "fs"
                      ],
                      "id": "tokio 1.25.0",
                      "target_name": "tokio"
                    }
                  ]
                }
              }
            },
            "tokio 1.25.0": {
              "common": {
                "build_deps": [
                  {
                    "features": [],
                    "id": "autocfg 1.1.0",
                    "target_name": "autocfg"
                  }
                ],
                "deps": [
                  {
                    "features": [],
                    "id": "pin-project-lite 0.2.9",
                    "target_name": "pin_project_lite"
                  }
                ],
                "features": [
                  "default"
                ]
              },
              "selects": {
                "x86_64-apple-darwin": {
                  "features": [
                    "fs"
                  ]
                },
                "x86_64-unknown-linux-gnu": {
                  "features": [
                    "fs"
                  ]
                }
              }
            }
        }),
        metadata.workspace_metadata["cargo-bazel"]["resolver_metadata"],
    );
}

#[test]
fn feature_generator_workspace() {
    if should_skip_test() {
        eprintln!("Skipping!");
        return;
    }

    let r = runfiles::Runfiles::create().unwrap();
    let metadata = run(
        "workspace_test",
        HashMap::from([
            (
                runfiles::rlocation!(
                    r,
                    "rules_rust/crate_universe/test_data/metadata/workspace/Cargo.toml"
                )
                .unwrap()
                .to_string_lossy()
                .to_string(),
                "//:test_input".to_string(),
            ),
            (
                runfiles::rlocation!(
                    r,
                    "rules_rust/crate_universe/test_data/metadata/workspace/child/Cargo.toml"
                )
                .unwrap()
                .to_string_lossy()
                .to_string(),
                "//crate_universe:test_data/metadata/workspace/child/Cargo.toml".to_string(),
            ),
        ]),
        "rules_rust/crate_universe/test_data/metadata/workspace/Cargo.lock",
    );

    assert!(
        !metadata.workspace_metadata["cargo-bazel"]["resolver_metadata"]["wgpu 0.14.0"].is_null()
    );
}

#[test]
fn feature_generator_crate_combined_features() {
    if should_skip_test() {
        eprintln!("Skipping!");
        return;
    }

    let r = runfiles::Runfiles::create().unwrap();
    let metadata = run(
        "crate_combined_features",
        HashMap::from([(
            runfiles::rlocation!(
                r,
                "rules_rust/crate_universe/test_data/metadata/crate_combined_features/Cargo.toml"
            )
            .unwrap()
            .to_string_lossy()
            .to_string(),
            "//:test_input".to_string(),
        )]),
        "rules_rust/crate_universe/test_data/metadata/crate_combined_features/Cargo.lock",
    );

    // serde appears twice in the list of dependencies, with and without derive features

    assert_eq!(
        json!({
            "common": {
                "features": [
                "default",
                "derive",
                "serde_derive",
                "std"
                ],
                "proc_macro_deps": [
                {
                    "features": [
                    "default"
                    ],
                    "id": "serde_derive 1.0.158",
                    "optional": true,
                    "target_name": "serde_derive"
                }
                ]
            },
            "selects": {}
        }),
        metadata.workspace_metadata["cargo-bazel"]["resolver_metadata"]["serde 1.0.158"],
    );
}

// See crate_universe/test_data/metadata/target_cfg_features/Cargo.toml for input.
#[test]
fn resolver_2_deps() {
    if should_skip_test() {
        eprintln!("Skipping!");
        return;
    }

    let r = runfiles::Runfiles::create().unwrap();
    let metadata = run(
        "resolver_2_deps_test",
        HashMap::from([(
            runfiles::rlocation!(
                r,
                "rules_rust/crate_universe/test_data/metadata/resolver_2_deps/Cargo.toml"
            )
            .unwrap()
            .to_string_lossy()
            .to_string(),
            "//:test_input".to_string(),
        )]),
        "rules_rust/crate_universe/test_data/metadata/resolver_2_deps/Cargo.lock",
    );

    assert_eq!(
        json!({
            "common": {
              "deps": [
                {
                  "features": [
                    "default"
                  ],
                  "id": "bytes 1.6.0",
                  "optional": true,
                  "target_name": "bytes"
                },
                {
                  "features": [],
                  "id": "pin-project-lite 0.2.14",
                  "target_name": "pin_project_lite"
                }
              ],
              "features": [
                "bytes",
                "default",
                "io-util"
              ]
            },
            // Note that there is no `wasm32-unknown-unknown` entry since all it's dependencies
            // are common. Also note that `mio` is unique to these platforms as it's something
            // that should be excluded from Wasm platforms.
            "selects": {
              "x86_64-apple-darwin": {
                "deps": [
                  {
                    "features": [
                      "default"
                    ],
                    "id": "libc 0.2.153",
                    "optional": true,
                    "target_name": "libc"
                  },
                  {
                    "features": [
                      "net",
                      "os-ext",
                      "os-poll"
                    ],
                    "id": "mio 0.8.11",
                    "optional": true,
                    "target_name": "mio"
                  },
                  {
                    "features": [
                      "all"
                    ],
                    "id": "socket2 0.5.7",
                    "optional": true,
                    "target_name": "socket2"
                  }
                ],
                "features": [
                  "io-std",
                  "libc",
                  "net",
                  "rt",
                  "socket2",
                  "sync",
                  "time"
                ]
              },
              "x86_64-pc-windows-msvc": {
                "deps": [
                  {
                    "features": [
                      "net",
                      "os-ext",
                      "os-poll"
                    ],
                    "id": "mio 0.8.11",
                    "optional": true,
                    "target_name": "mio"
                  },
                  {
                    "features": [
                      "all"
                    ],
                    "id": "socket2 0.5.7",
                    "optional": true,
                    "target_name": "socket2"
                  },
                  {
                    "features": [
                      "Win32_Foundation",
                      "Win32_Security",
                      "Win32_Storage_FileSystem",
                      "Win32_System_Pipes",
                      "Win32_System_SystemServices",
                      "default"
                    ],
                    "id": "windows-sys 0.48.0",
                    "optional": true,
                    "target_name": "windows_sys"
                  }
                ],
                "features": [
                  "io-std",
                  "libc",
                  "net",
                  "rt",
                  "socket2",
                  "sync",
                  "time"
                ]
              },
              "x86_64-unknown-linux-gnu": {
                "deps": [
                  {
                    "features": [
                      "default"
                    ],
                    "id": "libc 0.2.153",
                    "optional": true,
                    "target_name": "libc"
                  },
                  {
                    "features": [
                      "net",
                      "os-ext",
                      "os-poll"
                    ],
                    "id": "mio 0.8.11",
                    "optional": true,
                    "target_name": "mio"
                  },
                  {
                    "features": [
                      "all"
                    ],
                    "id": "socket2 0.5.7",
                    "optional": true,
                    "target_name": "socket2"
                  }
                ],
                "features": [
                  "io-std",
                  "libc",
                  "net",
                  "rt",
                  "socket2",
                  "sync",
                  "time"
                ]
              }
            }
        }),
        metadata.workspace_metadata["cargo-bazel"]["resolver_metadata"]["tokio 1.37.0"],
    );

    assert_eq!(
        json!({
          // Note linux is not present since linux has no unique dependencies or features
          // for this crate.
          "common": {},
          "selects": {
            "wasm32-unknown-unknown": {
              "deps": [
                {
                  "features": [],
                  "id": "js-sys 0.3.69",
                  "target_name": "js_sys"
                },
                {
                  "features": [
                    "default"
                  ],
                  "id": "wasm-bindgen 0.2.92",
                  "target_name": "wasm_bindgen"
                }
              ]
            },
            "x86_64-apple-darwin": {
              "deps": [
                {
                  "features": [
                    "default"
                  ],
                  "id": "core-foundation-sys 0.8.6",
                  "target_name": "core_foundation_sys"
                }
              ]
            },
            "x86_64-pc-windows-msvc": {
              "deps": [
                {
                  "features": [
                    "default"
                  ],
                  "id": "windows-core 0.52.0",
                  "target_name": "windows_core"
                }
              ]
            }
          }
        }),
        metadata.workspace_metadata["cargo-bazel"]["resolver_metadata"]["iana-time-zone 0.1.60"],
    );
}

#[test]
fn host_specific_build_deps() {
    if should_skip_test() {
        eprintln!("Skipping!");
        return;
    }

    let r = runfiles::Runfiles::create().unwrap();

    let src_cargo_toml = runfiles::rlocation!(
        r,
        "rules_rust/crate_universe/test_data/metadata/host_specific_build_deps/Cargo.toml"
    )
    .unwrap();

    // Put Cargo.toml into writable directory structure and create target/ directory to verify that
    // cargo does not incorrectly cache rustc info in target/.rustc_info.json file.
    let scratch = tempfile::tempdir().unwrap();
    let cargo_toml = scratch.path().join("Cargo.toml");
    fs::copy(src_cargo_toml, &cargo_toml).unwrap();
    fs::create_dir(scratch.path().join("target")).unwrap();

    let metadata = run(
        "host_specific_build_deps",
        HashMap::from([(
            cargo_toml.to_string_lossy().to_string(),
            "//:test_input".to_string(),
        )]),
        "rules_rust/crate_universe/test_data/metadata/host_specific_build_deps/Cargo.lock",
    );

    assert_eq!(
        json!({
            "common": {},
            // Note that there is no `wasm32-unknown-unknown` or `x86_64-pc-windows-msvc` entry
            // since these platforms do not depend on `rustix`. The chain breaks due to the
            // conditions here: https://github.com/Stebalien/tempfile/blob/v3.11.0/Cargo.toml#L25-L33
            "selects": {
                "x86_64-apple-darwin": {
                    "deps": [
                        {
                            "features": [
                                "std"
                            ],
                            "id": "bitflags 2.6.0",
                            "target_name": "bitflags"
                        },
                        {
                            "alias": "libc_errno",
                            "features": [
                                "std"
                            ],
                            "id": "errno 0.3.9",
                            "target_name": "errno"
                        },
                        {
                            "features": [
                                "extra_traits",
                                "std"
                            ],
                            "id": "libc 0.2.158",
                            "target_name": "libc"
                        }
                    ],
                    "features": [
                        "alloc",
                        "default",
                        "fs",
                        "libc-extra-traits",
                        "std",
                        "use-libc-auxv"
                    ]
                },
                "x86_64-unknown-linux-gnu": {
                    "deps": [
                        {
                            "features": [
                                "std"
                            ],
                            "id": "bitflags 2.6.0",
                            "target_name": "bitflags"
                        },
                        {
                            "features": [
                                "elf",
                                "errno",
                                "general",
                                "ioctl",
                                "no_std"
                            ],
                            "id": "linux-raw-sys 0.4.14",
                            "target_name": "linux_raw_sys"
                        }
                    ],
                    "features": [
                        "alloc",
                        "default",
                        "fs",
                        "libc-extra-traits",
                        "std",
                        "use-libc-auxv"
                    ]
                }
            },
        }),
        metadata.workspace_metadata["cargo-bazel"]["resolver_metadata"]["rustix 0.38.36"],
    );

    assert_eq!(
        json!({
            "common": {},
            // Note that windows does not contain `rustix` and instead `windows-sys`.
            // This shows correct detection of exec platform constraints.
            "selects": {
                "x86_64-apple-darwin": {
                    "deps": [
                        {
                            "features": [],
                            "id": "cfg-if 1.0.0",
                            "target_name": "cfg_if"
                        },
                        {
                            "features": [
                                "default"
                            ],
                            "id": "fastrand 2.1.1",
                            "target_name": "fastrand"
                        },
                        {
                            "features": [
                                "std"
                            ],
                            "id": "once_cell 1.19.0",
                            "target_name": "once_cell"
                        },
                        {
                            "features": [
                                "default",
                                "fs"
                            ],
                            "id": "rustix 0.38.36",
                            "target_name": "rustix"
                        }
                    ]
                },
                "x86_64-pc-windows-msvc": {
                    "deps": [
                        {
                            "features": [],
                            "id": "cfg-if 1.0.0",
                            "target_name": "cfg_if"
                        },
                        {
                            "features": [
                                "default"
                            ],
                            "id": "fastrand 2.1.1",
                            "target_name": "fastrand"
                        },
                        {
                            "features": [
                                "std"
                            ],
                            "id": "once_cell 1.19.0",
                            "target_name": "once_cell"
                        },
                        {
                            "features": [
                                "Win32_Foundation",
                                "Win32_Storage_FileSystem",
                                "default"
                            ],
                            "id": "windows-sys 0.59.0",
                            "target_name": "windows_sys"
                        }
                    ]
                },
                "x86_64-unknown-linux-gnu": {
                    "deps": [
                        {
                            "features": [],
                            "id": "cfg-if 1.0.0",
                            "target_name": "cfg_if"
                        },
                        {
                            "features": [
                                "default"
                            ],
                            "id": "fastrand 2.1.1",
                            "target_name": "fastrand"
                        },
                        {
                            "features": [
                                "std"
                            ],
                            "id": "once_cell 1.19.0",
                            "target_name": "once_cell"
                        },
                        {
                            "features": [
                                "default",
                                "fs"
                            ],
                            "id": "rustix 0.38.36",
                            "target_name": "rustix"
                        }
                    ]
                }
            }
        }),
        metadata.workspace_metadata["cargo-bazel"]["resolver_metadata"]["tempfile 3.12.0"],
    );
}
