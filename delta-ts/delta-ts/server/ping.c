/** @file
 * @brief Server Group
 *
 * Verify the Delta API server liveness probe and empty pool listing.
 *
 * Copyright (C) 2025-2026 Interpretica Unipessoal Lda
 *
 * @author Maxim Menshikov <maxim.menshikov@interpretica.io>
 *
 * $Id: $
 */

/** @page server-ping Delta API ping test
 *
 * @objective Check that a freshly launched Delta API server answers a
 *            liveness probe and reports an empty node pool.
 *
 * @par Scenario:
 */

#define TE_TEST_NAME    "server/ping"

#include <time.h>

#include "te_config.h"
#include "tapi_test.h"
#include "tsapi_evo.h"
#include "delta_api.h"

/** Number of ping round-trips used to estimate latency. */
#define PING_ROUNDS 100

int
main(int argc, char **argv)
{
    delta_server     srv;
    delta_client     cli;
    char             resp[DELTA_API_LINE_MAX + 1];
    tsapi_evo_schart chart;
    struct timespec  start;
    struct timespec  end;
    double           avg_us;
    char             metric[64];
    int              i;

    TEST_START;

    tsapi_evo_analysis_hint("Delta API server: verify the liveness probe "
                            "and that a fresh node pool is empty.");

    TEST_STEP("Launch the Delta API server");
    CHECK_RC(delta_server_start(&srv));

    TEST_STEP("Open a client connection");
    CHECK_RC(delta_client_open(&srv, &cli));

    TEST_STEP("Send a ping and expect a pong");
    CHECK_RC(delta_client_call(&cli, "{\"op\":\"ping\"}",
                               resp, sizeof(resp)));
    if (!delta_response_is(resp, "{\"op\":\"pong\"}"))
        TEST_VERDICT("Ping request did not return a pong: %s", resp);

    TEST_STEP("Measure ping round-trip latency over %d rounds", PING_ROUNDS);
    clock_gettime(CLOCK_MONOTONIC, &start);
    for (i = 0; i < PING_ROUNDS; i++)
    {
        CHECK_RC(delta_client_call(&cli, "{\"op\":\"ping\"}",
                                   resp, sizeof(resp)));
        if (!delta_response_is(resp, "{\"op\":\"pong\"}"))
            TEST_VERDICT("Ping round %d did not return a pong: %s", i, resp);
    }
    clock_gettime(CLOCK_MONOTONIC, &end);

    avg_us = ((double)(end.tv_sec - start.tv_sec) * 1e6 +
              (double)(end.tv_nsec - start.tv_nsec) / 1e3) / PING_ROUNDS;
    RING("Average ping round-trip: %.2f us", avg_us);

    TEST_STEP("Record the ping latency in a chart");
    tsapi_evo_schart_init(&chart, "Delta API",
                          "Ping round-trip latency / Operation",
                          "0", "us");
    snprintf(metric, sizeof(metric), "%.2f", avg_us);
    tsapi_evo_schart_add_metric(&chart, "ping", metric, NULL);
    tsapi_evo_schart_print(&chart);
    tsapi_evo_schart_fini(&chart);

    TEST_STEP("Verify a fresh node pool lists no nodes");
    CHECK_RC(delta_client_call(&cli, "{\"op\":\"list_nodes\"}",
                               resp, sizeof(resp)));
    if (!delta_response_is(resp, "{\"op\":\"list_nodes\",\"result\":[]}"))
        TEST_VERDICT("A fresh node pool is not empty: %s", resp);

    TEST_SUCCESS;

cleanup:

    delta_client_close(&cli);
    delta_server_stop(&srv);

    TEST_END;
}
