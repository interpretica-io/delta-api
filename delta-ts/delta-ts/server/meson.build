# Copyright (C) 2025-2026 Interpretica Unipessoal Lda

# The Delta API server is a Rust binary built from the delta-api crate.
# It is compiled here and installed next to the server tests so that the
# tests can launch it.
cargo = find_program('cargo')
build_delta_server = find_program(
    meson.current_source_dir() / 'build_delta_server.sh')

# delta-api crate location: the suite lives inside the delta-api repository,
# two directories above the suite project root. The build script also honours
# the DELTA_API_SRC environment variable, which takes precedence.
delta_api_src = meson.project_source_root() / '..' / '..'

delta_server = custom_target('delta-server',
    output: 'delta-server',
    command: [ build_delta_server, cargo.full_path(), delta_api_src,
               '@OUTPUT@' ],
    build_by_default: true,
    install: true,
    install_dir: package_dir,
    install_mode: 'rwxr-xr-x',
)

tests = [
    'prologue',
    'epilogue',
    'ping',
    'node_lifecycle',
    'error_handling',
    'shared_state',
]

foreach test : tests
    test_exe = test
    test_c = test + '.c'
    package_tests_c += [ test_c ]
    executable(test_exe, test_c, install: true, install_dir: package_dir,
               dependencies: test_deps)
endforeach

install_data([ 'package.xml' ], install_dir: package_dir)
