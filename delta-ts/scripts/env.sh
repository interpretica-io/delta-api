#!/bin/bash
if [ ! -d "${TS_TOPDIR}" ] ; then
    echo "TS_TOPDIR is not set" 1>&2
    exit 1
fi

# Locate the Test Environment sources. Honour an explicit TE_BASE first,
# then fall back to well-known locations relative to the delta-api repo.
if [ -z "${TE_BASE}" ] ; then
    for candidate in "${DELTA_API_SRC}/test-environment" \
                     "${DELTA_API_SRC}/../test-environment" \
                     "${TS_TOPDIR}/../test-environment" ; do
        if [ -n "${candidate}" ] && [ -d "${candidate}" ] ; then
            export TE_BASE="$(cd "${candidate}" ; pwd -P)"
            break
        fi
    done
fi
