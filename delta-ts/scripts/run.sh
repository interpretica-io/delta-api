#!/bin/bash
RUNDIR="$(pwd -P)"
export TS_TOPDIR="$(cd "$(dirname "$(which "$0")")"/.. ; pwd -P)"
export TE_TS_DELTA=${TS_TOPDIR}/delta-ts

# The delta-ts suite lives inside the delta-api repository; the delta-api
# crate (with the delta-server source) is the parent directory of the suite.
if [ -z "${DELTA_API_SRC}" ] ; then
    export DELTA_API_SRC="$(cd "${TS_TOPDIR}/.." ; pwd -P)"
fi

function show_usage() {
    echo "Usage:"
    echo "    $0 [parameters] ..."
    echo
    echo "For the ease of use, you can use:"
    echo "    $0 guess [parameters] ..."
    echo "This variant expects the following folder structure:"
    echo " - delta-api (with the delta-ts suite inside it)"
    echo " - test-environment"
    echo
}

function run_fail() {
    echo "Error:" $@ >&2
    exit 1
}

if [ -f /tmp/te.dhcpd.leases ] ; then
    sudo rm /tmp/te.dhcpd*.leases
fi

is_docker="0"
if [ "$1" == "docker" ] ; then
    is_docker="1"
    shift 1
fi

if [ "$1" == "guess" ] ; then
    . ${TS_TOPDIR}/scripts/env.sh
    shift 1
fi

if [ "$is_docker" == "1" ] ; then
    export TE_DOCKER_FILE_DIR="${TS_TOPDIR}/scripts/docker/build"
    export TE_DOCKER_TAG="delta-ts-build"
    # DELTA_API_SRC contains the delta-ts suite, so mounting it is enough.
    export TE_DOCKER_MOUNT_PATHS="${DELTA_API_SRC}:${TE_BASE}"
    export TE_DOCKER_WORK_DIR="$(pwd)"
    export TE_DOCKER_ENV="TE_BASE:TS_TOPDIR:DELTA_API_SRC:TE_TS_DELTA"
    ${TS_TOPDIR}/scripts/docker_env.sh ${TS_TOPDIR}/scripts/run.sh $@
    exit $?
fi

if test -z "${TE_BASE}" ; then
    for candidate in "${DELTA_API_SRC}/test-environment" \
                     "${DELTA_API_SRC}/../test-environment" \
                     "${TS_TOPDIR}/../test-environment" ; do
        if [ -d "${candidate}" ] ; then
            export TE_BASE="$(cd "${candidate}" ; pwd -P)"
            break
        fi
    done
fi

if test -z "${TE_BASE}" ; then
    if test -e dispatcher.sh ; then
        export TE_BASE="${RUNDIR}"
    elif test -e "${TS_TOPDIR}/dispatcher.sh" ; then
        export TE_BASE="${TS_TOPDIR}"
    else
        show_usage
        run_fail "Path to TE sources MUST be specified in TE_BASE"
    fi
fi

usage() {
    cat <<EOF
USAGE: run.sh [run.sh options] [dispatcher.sh options]
Options:
    --cfg=<CFG>             Configuration to be used
EOF
${TE_BASE}/dispatcher.sh --help
exit 1
}

TS_OPTS=""
TS_CFG="localhost"
while test -n "$1" ; do
    case $1 in
        --help)
            usage
            ;;
        --cfg=*)
            TS_CFG=${1#--cfg=}
            ;;
        *)
            TS_OPTS+="$1 "
            ;;
    esac
    shift 1
done

source "${TE_BASE}/scripts/lib.meta"

te_meta_test_suite delta-ts ATS
te_meta_add "CFG" "$TS_CFG"

test -z "${TS_CFG}" || TS_OPTS+="--opts=run.conf.${TS_CFG} "

echo "Config is ${TS_CFG}"

if test -z "${TE_BUILD}" ; then
    if test "${TS_TOPDIR}" = "${RUNDIR}" ; then
        TE_BUILD="${TS_TOPDIR}/build"
        mkdir -p build
    else
        TE_BUILD="${RUNDIR}"
    fi
    export TE_BUILD
fi

if [ "${TE_WORKSPACE_DIR}" == "" ] ; then
    export TE_WORKSPACE_DIR="${TE_BUILD}/workspace"
fi

TS_CONF_DIRS=
TS_CONF_DIRS+="\"${TS_TOPDIR}\"/conf:"

TS_DEFAULT_OPTS=

TS_DEFAULT_OPTS+="--conf-dirs=${TS_CONF_DIRS} "
TS_DEFAULT_OPTS+="--build-parallel "
TS_DEFAULT_OPTS+="--trc-db=\"${TS_TOPDIR}\"/conf/trc.xml "
TS_DEFAULT_OPTS+="--trc-tag=trc_test1 --trc-tag=trc_test2 "

export TE_TMP="${TS_TOPDIR}/tmp"
mkdir -p "${TE_TMP}"

export TE_TS_SRC="${TS_TOPDIR}/delta-ts"

eval "${TE_BASE}/dispatcher.sh ${TS_DEFAULT_OPTS} ${TS_OPTS} " \
    "--script=scripts/ta-def"
if [ "$?" == "0" ] ; then
    echo OK
else
    echo FAIL
fi

mkdir -p ${TS_TOPDIR}/meson-logs
rm -rf ${TS_TOPDIR}/meson-logs/*
cp -R /tmp/te_ws*/build/meson-logs/* ${TS_TOPDIR}/meson-logs/ 2>/dev/null
