/** @file
 * @brief Server Group
 *
 * Verify the Delta API server shares pool state and pipelines requests.
 *
 * Copyright (C) 2025-2026 Interpretica Unipessoal Lda
 *
 * @author Maxim Menshikov <maxim.menshikov@interpretica.io>
 *
 * $Id: $
 */

/** @page server-shared_state Delta API shared state test
 *
 * @objective Check that the Delta API server shares the node pool across
 *            connections and answers pipelined requests in order.
 *
 * @par Scenario:
 */

#define TE_TEST_NAME    "server/shared_state"

#include "te_config.h"
#include "tapi_test.h"
#include "tsapi_evo.h"
#include "delta_api.h"

int
main(int argc, char **argv)
{
    delta_server srv;
    delta_client writer;
    delta_client reader;
    char         resp[DELTA_API_LINE_MAX + 1];

    TEST_START;

    tsapi_evo_analysis_hint("Delta API server: verify that the node pool is "
                            "shared across connections and that pipelined "
                            "requests are answered in order.");

    TEST_STEP("Launch the Delta API server");
    CHECK_RC(delta_server_start(&srv));

    TEST_STEP("Open the first client connection and register a node");
    CHECK_RC(delta_client_open(&srv, &writer));
    CHECK_RC(delta_client_call(&writer,
        "{\"op\":\"add\",\"name\":\"shared\",\"fqdn\":\"h\"}",
        resp, sizeof(resp)));
    if (!delta_response_is(resp, "{\"op\":\"add\",\"result\":\"Ok\"}"))
        TEST_VERDICT("Registering a node on the first connection failed: %s",
                     resp);

    TEST_STEP("A second connection sees the node added by the first");
    CHECK_RC(delta_client_open(&srv, &reader));
    CHECK_RC(delta_client_call(&reader, "{\"op\":\"list_nodes\"}",
                               resp, sizeof(resp)));
    if (!delta_response_is(resp,
            "{\"op\":\"list_nodes\",\"result\":[\"shared\"]}"))
    {
        TEST_VERDICT("The node pool is not shared across connections: %s",
                     resp);
    }

    TEST_STEP("Pipeline three requests on one connection");
    CHECK_RC(delta_client_send(&reader, "{\"op\":\"ping\"}"));
    CHECK_RC(delta_client_send(&reader,
        "{\"op\":\"add\",\"name\":\"p\",\"fqdn\":\"h\"}"));
    CHECK_RC(delta_client_send(&reader, "{\"op\":\"list_nodes\"}"));

    TEST_STEP("The pipelined responses are returned in order");
    CHECK_RC(delta_client_recv(&reader, resp, sizeof(resp)));
    if (!delta_response_is(resp, "{\"op\":\"pong\"}"))
        TEST_VERDICT("The first pipelined response is wrong: %s", resp);

    CHECK_RC(delta_client_recv(&reader, resp, sizeof(resp)));
    if (!delta_response_is(resp, "{\"op\":\"add\",\"result\":\"Ok\"}"))
        TEST_VERDICT("The second pipelined response is wrong: %s", resp);

    CHECK_RC(delta_client_recv(&reader, resp, sizeof(resp)));
    if (!delta_response_is(resp,
            "{\"op\":\"list_nodes\",\"result\":[\"p\",\"shared\"]}"))
    {
        TEST_VERDICT("The third pipelined response is wrong: %s", resp);
    }

    TEST_SUCCESS;

cleanup:

    delta_client_close(&reader);
    delta_client_close(&writer);
    delta_server_stop(&srv);

    TEST_END;
}
