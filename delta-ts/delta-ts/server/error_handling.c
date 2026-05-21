/** @file
 * @brief Server Group
 *
 * Verify the Delta API server error paths and protocol robustness.
 *
 * Copyright (C) 2025-2026 Interpretica Unipessoal Lda
 *
 * @author Maxim Menshikov <maxim.menshikov@interpretica.io>
 *
 * $Id: $
 */

/** @page server-error_handling Delta API error handling test
 *
 * @objective Check that the Delta API server rejects malformed requests and
 *            unknown nodes without dropping the connection.
 *
 * @par Scenario:
 */

#define TE_TEST_NAME    "server/error_handling"

#include "te_config.h"
#include "tapi_test.h"
#include "tsapi_evo.h"
#include "delta_api.h"

/**
 * Send @p request and fail the test unless the response contains @p substr.
 */
static void
expect_contains(delta_client *cli, const char *request,
                const char *substr, const char *verdict)
{
    char resp[DELTA_API_LINE_MAX + 1];

    CHECK_RC(delta_client_call(cli, request, resp, sizeof(resp)));
    if (!delta_response_has(resp, substr))
        TEST_VERDICT("%s (got: %s)", verdict, resp);
}

int
main(int argc, char **argv)
{
    delta_server srv;
    delta_client cli;

    TEST_START;

    tsapi_evo_analysis_hint("Delta API server: verify that malformed "
                            "requests and unknown nodes are reported as "
                            "errors and the connection survives.");

    TEST_STEP("Launch the Delta API server");
    CHECK_RC(delta_server_start(&srv));

    TEST_STEP("Open a client connection");
    CHECK_RC(delta_client_open(&srv, &cli));

    TEST_STEP("A non-JSON line yields an error response");
    expect_contains(&cli, "this is not json", "\"op\":\"error\"",
                    "A non-JSON request was not reported as an error");

    TEST_STEP("An unknown operation yields an error response");
    expect_contains(&cli, "{\"op\":\"teleport\",\"name\":\"n1\"}",
                    "\"op\":\"error\"",
                    "An unknown operation was not reported as an error");

    TEST_STEP("A request with a missing field yields an error response");
    expect_contains(&cli, "{\"op\":\"add\",\"name\":\"n1\"}",
                    "\"op\":\"error\"",
                    "A request missing a field was not reported as an error");

    TEST_STEP("The connection survives errors and still serves requests");
    expect_contains(&cli, "{\"op\":\"ping\"}", "{\"op\":\"pong\"}",
                    "The connection did not survive malformed requests");

    TEST_STEP("Operating on an unknown node reports NodeNotFound");
    expect_contains(&cli, "{\"op\":\"connect\",\"name\":\"ghost\"}",
                    "\"result\":\"NodeNotFound\"",
                    "Connecting to an unknown node was not rejected");
    expect_contains(&cli, "{\"op\":\"disconnect\",\"name\":\"ghost\"}",
                    "\"result\":\"NodeNotFound\"",
                    "Disconnecting an unknown node was not rejected");
    expect_contains(&cli, "{\"op\":\"run\",\"name\":\"ghost\",\"subject\":\"Sa\"}",
                    "\"result\":\"NodeNotFound\"",
                    "Running on an unknown node was not rejected");

    TEST_STEP("Deploying the Delta subject is rejected as invalid");
    expect_contains(&cli,
                    "{\"op\":\"deploy\",\"name\":\"ghost\",\"subject\":\"Delta\"}",
                    "\"result\":\"InvalidArgument\"",
                    "Deploying the Delta subject was not rejected");

    TEST_STEP("Deploying a Sa subject onto an unknown node reports "
              "NodeNotFound");
    expect_contains(&cli,
                    "{\"op\":\"deploy\",\"name\":\"ghost\",\"subject\":\"Sa\"}",
                    "\"result\":\"NodeNotFound\"",
                    "Deploying onto an unknown node was not rejected");

    TEST_SUCCESS;

cleanup:

    delta_client_close(&cli);
    delta_server_stop(&srv);

    TEST_END;
}
