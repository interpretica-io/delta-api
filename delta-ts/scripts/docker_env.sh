#!/bin/bash
# Docker-based environment for Test Environment build & run
TE_DOCKER_SOCK_DEFAULT="/var/run/docker.sock"
docker_sock="${TE_DOCKER_SOCK:-${TE_DOCKER_SOCK_DEFAULT}}"

usage() {
    echo "This script builds & runs a docker container suitable for Test Environment build."
    echo "The script tries to mimic current user and steal away his permissions"
    echo
    echo "Usage:"
    echo "$0 command [arguments...]"
    echo
    echo "Mandatory environment variables:"
    echo " - TE_DOCKER_FILE_DIR=path - path where Dockerfile is contained"
    echo " - TE_DOCKER_TAG=tag - image tag"
    echo " - TE_DOCKER_MOUNT_PATHS=path1:path2:path3... - paths to mount"
    echo
    echo "Optional environment variables:"
    echo " - TE_DOCKER_SOCK=path - path to docker control socket (default: ${TE_DOCKER_SOCK_DEFAULT})"
    echo " - TE_DOCKER_WORK_DIR=path - working folder, must be present across mount paths"
    echo " - TE_DOCKER_ENV=envvar1:envvar2:envvar3... - environment variables to pass to container"
}

# Container platform. By default Docker picks the host-native platform,
# which avoids slow emulation. Set TE_DOCKER_PLATFORM (e.g. linux/amd64) to
# force a specific one.
PLATFORM_ARG=""
if [ -n "${TE_DOCKER_PLATFORM}" ] ; then
    PLATFORM_ARG="--platform ${TE_DOCKER_PLATFORM}"
fi

printenv | grep TE_DOCKER
if [ -z "${TE_DOCKER_MOUNT_PATHS}" ] ||
   [ -z "${TE_DOCKER_FILE_DIR}" ] ||
   [ -z "${TE_DOCKER_TAG}" ] ; then
    echo "Error: mandatory environment variables are not set" 1>&2
    usage
    exit 1
fi

if [ "$(uname -s)" != "Darwin" ] ; then
    # Get and verify docker group ID
    docker_group_id="$(getent group docker)"
    if [ "$?" != "0" ] ; then
        echo "Error: 'docker' group must be present. Is docker configured?" 1>&2
        exit 2
    fi
else
    docker_group_id="$(id -g ${USER})"
fi

# Form mounts list appropriate for Docker
MOUNTS=""
IFS=':' read -ra ARRAY <<< "${TE_DOCKER_MOUNT_PATHS}"
for path in "${ARRAY[@]}"; do
    MOUNTS="${MOUNTS} -v ${path}:${path}"
done

ENV_VARS=""
IFS=':' read -ra ARRAY <<< "${TE_DOCKER_ENV}"
for env_var in "${ARRAY[@]}"; do
    ENV_VARS="${ENV_VARS} -e ${env_var}"
done

# Build docker image
pushd "${TE_DOCKER_FILE_DIR}"
docker build ${PLATFORM_ARG} -t ${USER}/${TE_DOCKER_TAG} \
       --build-arg user=${USER} \
       --build-arg uid=$(id -u ${USER}) \
       --build-arg gid=$(id -g ${USER}) \
       --build-arg docker_gid=$(cut -d: -f3 <<< "${docker_group_id}") \
       .
if [ "$?" != "0" ] ; then
    echo "Error: failed to build container" 1>&2
    exit 3
fi
popd

# Run docker image
docker run ${PLATFORM_ARG} --rm -i \
       --network host \
       --user=$(id -u):$(id -g) \
       --hostname=$(hostname)-te \
       --group-add docker \
       ${MOUNTS} \
       ${ENV_VARS} \
       ${TE_DOCKER_WORK_DIR:+-w "${TE_DOCKER_WORK_DIR}"} \
       -v ~/.ssh:/home/${USER}/.ssh \
       -v ${docker_sock}:${docker_sock} \
       ${USER}/${TE_DOCKER_TAG} $@
