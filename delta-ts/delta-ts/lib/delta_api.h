/** @file
 * @brief TS API - Delta API client
 *
 * Helpers to launch the Delta API network server and drive its
 * newline-delimited JSON protocol from a test.
 *
 * Copyright (C) 2025-2026 Interpretica Unipessoal Lda
 *
 * @author Maxim Menshikov <maxim.menshikov@interpretica.io>
 */

#ifndef __DELTA_API_H__
#define __DELTA_API_H__

#include "te_config.h"

#include <stddef.h>
#include <sys/types.h>

#include "te_defs.h"
#include "te_errno.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @defgroup delta_api Delta API client
 * @ingroup delta_ts
 * @{
 *
 * @brief Launch and exercise the Delta API network server.
 *
 * The Delta API server (@c delta-server) wraps a node pool and exposes it
 * over TCP using a newline-delimited JSON protocol: each request is one JSON
 * object terminated by @c '\n', and each response is one JSON object
 * terminated by @c '\n'.
 *
 * This module provides:
 * - launching a private @c delta-server instance bound to an ephemeral port;
 * - opening client connections to it;
 * - sending requests and reading line-framed responses;
 * - small helpers to match responses.
 */

/** Maximum length of a single Delta API protocol line, terminator excluded. */
#define DELTA_API_LINE_MAX 4096

/** A running @c delta-server instance. */
typedef struct delta_server {
    pid_t pid;        /**< Server process identifier (0 if not running). */
    int   port;       /**< TCP port the server is listening on. */
    char  addr[64];   /**< "host:port" the server is listening on. */
} delta_server;

/** A client connection to a @c delta-server instance. */
typedef struct delta_client {
    int    fd;                          /**< Connected socket descriptor. */
    char   buf[DELTA_API_LINE_MAX + 1]; /**< Pending received bytes. */
    size_t buf_len;                     /**< Number of valid bytes in @p buf. */
} delta_client;

/**
 * Launch a private @c delta-server instance bound to an ephemeral
 * loopback port.
 *
 * The binary is located via the @c DELTA_SERVER_BIN environment variable,
 * or, when it is not set, next to the running test executable. The function
 * returns only once the server has reported the port it listens on.
 *
 * @param[out] srv  Server handle to initialize.
 *
 * @return Status code.
 *
 * @sa delta_server_stop
 */
extern te_errno delta_server_start(delta_server *srv);

/**
 * Terminate a @c delta-server instance and reap it.
 *
 * Safe to call on a zeroed or already-stopped handle.
 *
 * @param[in,out] srv  Server handle.
 *
 * @sa delta_server_start
 */
extern void delta_server_stop(delta_server *srv);

/**
 * Open a client connection to a running @c delta-server.
 *
 * @param[in]  srv  Server handle.
 * @param[out] cli  Client handle to initialize.
 *
 * @return Status code.
 *
 * @sa delta_client_close
 */
extern te_errno delta_client_open(const delta_server *srv,
                                  delta_client *cli);

/**
 * Close a client connection.
 *
 * Safe to call on a zeroed or already-closed handle.
 *
 * @param[in,out] cli  Client handle.
 */
extern void delta_client_close(delta_client *cli);

/**
 * Send one request line to the server.
 *
 * A newline terminator is appended automatically; @p request must not
 * contain one.
 *
 * @param[in,out] cli      Client handle.
 * @param[in]     request  JSON request, without the trailing newline.
 *
 * @return Status code.
 */
extern te_errno delta_client_send(delta_client *cli, const char *request);

/**
 * Receive one response line from the server.
 *
 * The trailing newline is stripped and @p line is NUL-terminated.
 *
 * @param[in,out] cli       Client handle.
 * @param[out]    line      Buffer for the response line.
 * @param[in]     line_len  Size of @p line in bytes.
 *
 * @return Status code.
 */
extern te_errno delta_client_recv(delta_client *cli, char *line,
                                  size_t line_len);

/**
 * Send a request and receive the matching response.
 *
 * Convenience wrapper around delta_client_send() and delta_client_recv().
 *
 * @param[in,out] cli           Client handle.
 * @param[in]     request       JSON request, without the trailing newline.
 * @param[out]    response      Buffer for the response line.
 * @param[in]     response_len  Size of @p response in bytes.
 *
 * @return Status code.
 */
extern te_errno delta_client_call(delta_client *cli, const char *request,
                                  char *response, size_t response_len);

/**
 * Check whether a response is exactly equal to an expected string.
 *
 * @param[in] actual    Response received from the server.
 * @param[in] expected  Expected response.
 *
 * @return @c true if the strings are equal.
 */
extern bool delta_response_is(const char *actual, const char *expected);

/**
 * Check whether a response contains a substring.
 *
 * @param[in] actual  Response received from the server.
 * @param[in] substr  Substring to look for.
 *
 * @return @c true if @p substr is found in @p actual.
 */
extern bool delta_response_has(const char *actual, const char *substr);

/** @} */ /* end of delta_api */

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* !__DELTA_API_H__ */
