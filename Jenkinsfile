/* Build-verification pipeline for delta-api. No artifacts are produced — this
   only checks that the crate formats, builds and tests cleanly. */
pipeline {
  agent {
    dockerfile {
      filename 'Dockerfile'
      dir 'tools/build-env'
    }
  }

  environment {
    /* delta-api's build.rs builds the native libasp. Use the in-image cmake
       toolchain rather than asp's Docker image, so no docker-in-docker is
       needed on the agent. */
    ASP_NATIVE_BUILD = '1'
  }

  stages {
    stage('Format') {
      steps {
        /* Mark the workspace safe — uid manipulations can make Git wary. */
        sh 'git config --global --add safe.directory "*"'
        sh 'cargo fmt -- --check'
      }
    }
    stage('Build') {
      /* --all-features exercises object_model + server alongside the always-on
         asp client, so the whole crate (and the delta-server bin) is compiled. */
      steps {
        sh 'cargo build --all-features'
      }
    }
    stage('Test') {
      steps {
        sh 'cargo test --all-features'
      }
    }
  }
}
