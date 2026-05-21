/** @file
 * @brief Test Suite
 *
 * Test Suite epilogue.
 *
 * Copyright (C) 2025-2026 Interpretica Unipessoal Lda
 *
 * @author Maxim Menshikov <maxim.menshikov@interpretica.io>
 *
 * $Id: $
 */

#define TE_TEST_NAME    "epilogue"

#include "te_config.h"
#include "tapi_test.h"
#include "tsapi_evo.h"

int
main(int argc, char **argv)
{
    TEST_START;

    tsapi_evo_analysis_hint("Delta API test suite epilogue. "
                            "Finalizing the test environment.");

    TEST_STEP("Finalize test suite");
    /* No cleanup actions are required for this test suite. */

    TEST_SUCCESS;

cleanup:

    TEST_END;
}
