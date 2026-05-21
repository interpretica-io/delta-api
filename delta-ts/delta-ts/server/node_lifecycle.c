/** @file
 * @brief Server Group
 *
 * Verify the Delta API node registration lifecycle and lookups.
 *
 * Copyright (C) 2025-2026 Interpretica Unipessoal Lda
 *
 * @author Maxim Menshikov <maxim.menshikov@interpretica.io>
 *
 * $Id: $
 */

/** @page server-node_lifecycle Delta API node lifecycle test
 *
 * @objective Check node registration, listing, status lookups and removal
 *            against the Delta API server.
 *
 * @par Scenario:
 */

#define TE_TEST_NAME    "server/node_lifecycle"

#include "te_config.h"
#include "tapi_test.h"
#include "tsapi_evo.h"
#include "delta_api.h"

/**
 * Send @p request, then fail the test with @p verdict unless the response
 * is exactly @p expected.
 */
static void
expect_response(delta_client *cli, const char *request,
                const char *expected, const char *verdict)
{
    char resp[DELTA_API_LINE_MAX + 1];

    CHECK_RC(delta_client_call(cli, request, resp, sizeof(resp)));
    if (!delta_response_is(resp, expected))
        TEST_VERDICT("%s (got: %s)", verdict, resp);
}

int
main(int argc, char **argv)
{
    delta_server srv;
    delta_client cli;
    char         resp[DELTA_API_LINE_MAX + 1];

    TEST_START;

    tsapi_evo_analysis_hint("Delta API server: verify the node registration "
                            "lifecycle - add, list, status, remove.");

    TEST_STEP("Launch the Delta API server");
    CHECK_RC(delta_server_start(&srv));

    TEST_STEP("Open a client connection");
    CHECK_RC(delta_client_open(&srv, &cli));

    TEST_STEP("Register node 'n1'");
    expect_response(&cli,
        "{\"op\":\"add\",\"name\":\"n1\",\"fqdn\":\"host.example\","
        "\"params\":{\"Username\":\"u\"}}",
        "{\"op\":\"add\",\"result\":\"Ok\"}",
        "Registering a fresh node failed");

    TEST_STEP("Registering 'n1' again is rejected");
    expect_response(&cli,
        "{\"op\":\"add\",\"name\":\"n1\",\"fqdn\":\"host.example\"}",
        "{\"op\":\"add\",\"result\":\"NodeAlreadyExists\"}",
        "Duplicate node registration was not rejected");

    TEST_STEP("Register a second node 'a0'");
    expect_response(&cli,
        "{\"op\":\"add\",\"name\":\"a0\",\"fqdn\":\"other.example\"}",
        "{\"op\":\"add\",\"result\":\"Ok\"}",
        "Registering a second node failed");

    TEST_STEP("The node listing is sorted");
    expect_response(&cli, "{\"op\":\"list_nodes\"}",
        "{\"op\":\"list_nodes\",\"result\":[\"a0\",\"n1\"]}",
        "The node listing is missing nodes or is not sorted");

    TEST_STEP("A freshly registered node is not connected");
    CHECK_RC(delta_client_call(&cli, "{\"op\":\"is_connected\",\"name\":\"n1\"}",
                               resp, sizeof(resp)));
    if (!delta_response_has(resp, "\"op\":\"is_connected\"") ||
        !delta_response_has(resp, "\"connected\":false"))
    {
        TEST_VERDICT("A fresh node is reported as connected: %s", resp);
    }

    TEST_STEP("The liveness probe reports a not-alive Sa subject");
    CHECK_RC(delta_client_call(&cli, "{\"op\":\"is_alive\",\"name\":\"n1\"}",
                               resp, sizeof(resp)));
    if (!delta_response_has(resp, "\"op\":\"is_alive\"") ||
        !delta_response_has(resp, "\"alive\":false"))
    {
        TEST_VERDICT("The liveness probe of a fresh node is wrong: %s", resp);
    }

    TEST_STEP("Removing 'n1' succeeds once");
    expect_response(&cli, "{\"op\":\"remove\",\"name\":\"n1\"}",
        "{\"op\":\"remove\",\"result\":\"Ok\"}",
        "Removing a registered node failed");

    TEST_STEP("Removing 'n1' a second time is rejected");
    expect_response(&cli, "{\"op\":\"remove\",\"name\":\"n1\"}",
        "{\"op\":\"remove\",\"result\":\"NodeNotFound\"}",
        "Removing an unknown node was not rejected");

    TEST_STEP("Only the surviving node 'a0' is listed");
    expect_response(&cli, "{\"op\":\"list_nodes\"}",
        "{\"op\":\"list_nodes\",\"result\":[\"a0\"]}",
        "The node listing is wrong after removal");

    TEST_SUCCESS;

cleanup:

    delta_client_close(&cli);
    delta_server_stop(&srv);

    TEST_END;
}
