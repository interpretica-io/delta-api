/** @file
 * @brief Server Group
 *
 * Server test group prologue.
 *
 * Copyright (C) 2025-2026 Interpretica Unipessoal Lda
 *
 * @author Maxim Menshikov <maxim.menshikov@interpretica.io>
 *
 * $Id: $
 */

#define TE_TEST_NAME    "server/prologue"

#include "te_config.h"
#include "tapi_test.h"
#include "tsapi_evo.h"
#include "delta_api.h"

int
main(int argc, char **argv)
{
    delta_server srv;

    TEST_START;

    tsapi_evo_analysis_hint("Delta API server group prologue. "
                            "Verifying the delta-server binary can be "
                            "launched.");

    TEST_STEP("Launch the Delta API server");
    CHECK_RC(delta_server_start(&srv));

    TEST_STEP("Stop the Delta API server");
    delta_server_stop(&srv);

    TEST_SUCCESS;

cleanup:

    TEST_END;
}
