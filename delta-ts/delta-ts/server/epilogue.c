/** @file
 * @brief Server Group
 *
 * Server test group epilogue.
 *
 * Copyright (C) 2025-2026 Interpretica Unipessoal Lda
 *
 * @author Maxim Menshikov <maxim.menshikov@interpretica.io>
 *
 * $Id: $
 */

#define TE_TEST_NAME    "server/epilogue"

#include "te_config.h"
#include "tapi_test.h"
#include "tsapi_evo.h"

int
main(int argc, char **argv)
{
    TEST_START;

    tsapi_evo_analysis_hint("Delta API server group epilogue. "
                            "Cleaning up after the server tests.");

    TEST_STEP("Finalize server test group");
    /* Each test owns and stops its own server instance. */

    TEST_SUCCESS;

cleanup:

    TEST_END;
}
