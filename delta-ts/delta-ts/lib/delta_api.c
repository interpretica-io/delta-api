/** @file
 * @brief TS API - Delta API client
 *
 * Implementation of the Delta API server launcher and protocol client.
 *
 * Copyright (C) 2025-2026 Interpretica Unipessoal Lda
 *
 * @author Maxim Menshikov <maxim.menshikov@interpretica.io>
 */

#define TE_LGR_USER "Delta API"

#include "te_config.h"

#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <signal.h>
#include <poll.h>
#include <errno.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <netinet/in.h>

#include "tapi_test.h"
#include "delta_api.h"

/** The line the server prints once it is bound and accepting. */
#define DELTA_SERVER_BANNER "listening on 127.0.0.1:"

/** How long to wait for the server to become ready, in milliseconds. */
#define DELTA_SERVER_READY_TIMEOUT_MS 15000

/** How long to wait for a single response line, in milliseconds. */
#define DELTA_CLIENT_RECV_TIMEOUT_MS 10000

/**
 * Resolve the absolute path of the @c delta-server binary.
 *
 * Prefers the @c DELTA_SERVER_BIN environment variable; otherwise looks for
 * a @c delta-server binary installed next to the running test executable.
 */
static te_errno
delta_server_locate(char *path, size_t len)
{
    const char *env = getenv("DELTA_SERVER_BIN");
    char        self[PATH_MAX];
    ssize_t     n;
    char       *slash;

    if (env != NULL && env[0] != '\0')
    {
        if ((size_t)snprintf(path, len, "%s", env) >= len)
            return TE_RC(TE_TAPI, TE_ESMALLBUF);
        return 0;
    }

    n = readlink("/proc/self/exe", self, sizeof(self) - 1);
    if (n < 0)
    {
        ERROR("Cannot resolve the test executable path: %s", strerror(errno));
        return TE_OS_RC(TE_TAPI, errno);
    }
    self[n] = '\0';

    slash = strrchr(self, '/');
    if (slash == NULL)
        return TE_RC(TE_TAPI, TE_EFAIL);
    *slash = '\0';

    if ((size_t)snprintf(path, len, "%s/delta-server", self) >= len)
        return TE_RC(TE_TAPI, TE_ESMALLBUF);

    return 0;
}

/**
 * Wait until the server prints its banner to @p errfile and parse the port.
 */
static te_errno
delta_server_wait_ready(const char *errfile, pid_t pid, int *port)
{
    int attempt;

    for (attempt = 0; attempt * 50 < DELTA_SERVER_READY_TIMEOUT_MS; attempt++)
    {
        char    content[1024];
        char   *banner;
        int     fd;
        ssize_t n = 0;
        int     status;

        fd = open(errfile, O_RDONLY);
        if (fd >= 0)
        {
            n = read(fd, content, sizeof(content) - 1);
            close(fd);
        }

        if (n > 0)
        {
            content[n] = '\0';
            banner = strstr(content, DELTA_SERVER_BANNER);
            if (banner != NULL)
            {
                int value = atoi(banner + strlen(DELTA_SERVER_BANNER));

                if (value > 0)
                {
                    *port = value;
                    return 0;
                }
            }
        }

        if (waitpid(pid, &status, WNOHANG) == pid)
        {
            ERROR("delta-server exited before becoming ready");
            return TE_RC(TE_TAPI, TE_EFAIL);
        }

        usleep(50000);
    }

    ERROR("delta-server did not become ready within %d ms",
          DELTA_SERVER_READY_TIMEOUT_MS);
    return TE_RC(TE_TAPI, TE_ETIMEDOUT);
}

/* See the description in delta_api.h */
te_errno
delta_server_start(delta_server *srv)
{
    char     bin[PATH_MAX];
    char     errfile[] = "/tmp/delta_server_XXXXXX";
    int      errfd;
    pid_t    pid;
    int      port = 0;
    te_errno rc;

    memset(srv, 0, sizeof(*srv));

    rc = delta_server_locate(bin, sizeof(bin));
    if (rc != 0)
        return rc;

    if (access(bin, X_OK) != 0)
    {
        ERROR("delta-server binary is not executable at '%s': %s",
              bin, strerror(errno));
        return TE_OS_RC(TE_TAPI, errno);
    }

    errfd = mkstemp(errfile);
    if (errfd < 0)
    {
        ERROR("Cannot create a temporary file: %s", strerror(errno));
        return TE_OS_RC(TE_TAPI, errno);
    }

    pid = fork();
    if (pid < 0)
    {
        ERROR("fork() failed: %s", strerror(errno));
        close(errfd);
        unlink(errfile);
        return TE_OS_RC(TE_TAPI, errno);
    }

    if (pid == 0)
    {
        int devnull = open("/dev/null", O_RDWR);

        if (devnull >= 0)
        {
            dup2(devnull, STDIN_FILENO);
            dup2(devnull, STDOUT_FILENO);
            if (devnull > STDERR_FILENO)
                close(devnull);
        }
        dup2(errfd, STDERR_FILENO);
        if (errfd != STDERR_FILENO)
            close(errfd);

        execl(bin, bin, "127.0.0.1:0", (char *)NULL);
        _exit(127);
    }

    srv->pid = pid;

    rc = delta_server_wait_ready(errfile, pid, &port);
    close(errfd);
    unlink(errfile);

    if (rc != 0)
    {
        delta_server_stop(srv);
        return rc;
    }

    srv->port = port;
    snprintf(srv->addr, sizeof(srv->addr), "127.0.0.1:%d", port);
    RING("delta-server started (pid %d), listening on %s",
         (int)pid, srv->addr);

    return 0;
}

/* See the description in delta_api.h */
void
delta_server_stop(delta_server *srv)
{
    int status;
    int attempt;

    if (srv->pid <= 0)
        return;

    kill(srv->pid, SIGTERM);

    for (attempt = 0; attempt < 100; attempt++)
    {
        if (waitpid(srv->pid, &status, WNOHANG) == srv->pid)
        {
            RING("delta-server (pid %d) stopped", (int)srv->pid);
            srv->pid = 0;
            return;
        }
        usleep(20000);
    }

    WARN("delta-server (pid %d) did not exit on SIGTERM, sending SIGKILL",
         (int)srv->pid);
    kill(srv->pid, SIGKILL);
    waitpid(srv->pid, &status, 0);
    srv->pid = 0;
}

/* See the description in delta_api.h */
te_errno
delta_client_open(const delta_server *srv, delta_client *cli)
{
    struct sockaddr_in sa;
    int                attempt;

    memset(cli, 0, sizeof(*cli));
    cli->fd = -1;

    memset(&sa, 0, sizeof(sa));
    sa.sin_family = AF_INET;
    sa.sin_port = htons((uint16_t)srv->port);
    sa.sin_addr.s_addr = htonl(INADDR_LOOPBACK);

    for (attempt = 0; attempt < 50; attempt++)
    {
        int fd = socket(AF_INET, SOCK_STREAM, 0);

        if (fd < 0)
            return TE_OS_RC(TE_TAPI, errno);

        if (connect(fd, (struct sockaddr *)&sa, sizeof(sa)) == 0)
        {
            cli->fd = fd;
            return 0;
        }

        close(fd);
        usleep(20000);
    }

    ERROR("Cannot connect to delta-server at %s", srv->addr);
    return TE_RC(TE_TAPI, TE_ECONNREFUSED);
}

/* See the description in delta_api.h */
void
delta_client_close(delta_client *cli)
{
    if (cli->fd >= 0)
    {
        close(cli->fd);
        cli->fd = -1;
    }
    cli->buf_len = 0;
}

/* See the description in delta_api.h */
te_errno
delta_client_send(delta_client *cli, const char *request)
{
    const char *line[2] = { request, "\n" };
    size_t      idx;

    if (cli->fd < 0)
        return TE_RC(TE_TAPI, TE_EFAIL);

    for (idx = 0; idx < TE_ARRAY_LEN(line); idx++)
    {
        const char *p = line[idx];
        size_t      left = strlen(p);

        while (left > 0)
        {
            ssize_t sent = write(cli->fd, p, left);

            if (sent < 0)
            {
                if (errno == EINTR)
                    continue;
                ERROR("Failed to send a Delta API request: %s",
                      strerror(errno));
                return TE_OS_RC(TE_TAPI, errno);
            }
            p += sent;
            left -= (size_t)sent;
        }
    }

    return 0;
}

/* See the description in delta_api.h */
te_errno
delta_client_recv(delta_client *cli, char *line, size_t line_len)
{
    if (cli->fd < 0)
        return TE_RC(TE_TAPI, TE_EFAIL);

    for (;;)
    {
        char         *nl = memchr(cli->buf, '\n', cli->buf_len);
        struct pollfd pfd;
        ssize_t       n;
        int           pr;

        if (nl != NULL)
        {
            size_t len = (size_t)(nl - cli->buf);

            if (len + 1 > line_len)
                return TE_RC(TE_TAPI, TE_ESMALLBUF);

            memcpy(line, cli->buf, len);
            line[len] = '\0';

            cli->buf_len -= len + 1;
            memmove(cli->buf, nl + 1, cli->buf_len);

            return 0;
        }

        if (cli->buf_len >= sizeof(cli->buf))
        {
            ERROR("Delta API response line is too long");
            return TE_RC(TE_TAPI, TE_ESMALLBUF);
        }

        pfd.fd = cli->fd;
        pfd.events = POLLIN;
        pfd.revents = 0;

        pr = poll(&pfd, 1, DELTA_CLIENT_RECV_TIMEOUT_MS);
        if (pr < 0)
        {
            if (errno == EINTR)
                continue;
            return TE_OS_RC(TE_TAPI, errno);
        }
        if (pr == 0)
        {
            ERROR("Timed out waiting for a Delta API response");
            return TE_RC(TE_TAPI, TE_ETIMEDOUT);
        }

        n = read(cli->fd, cli->buf + cli->buf_len,
                 sizeof(cli->buf) - cli->buf_len);
        if (n < 0)
        {
            if (errno == EINTR)
                continue;
            return TE_OS_RC(TE_TAPI, errno);
        }
        if (n == 0)
        {
            ERROR("delta-server closed the connection unexpectedly");
            return TE_RC(TE_TAPI, TE_ECONNRESET);
        }

        cli->buf_len += (size_t)n;
    }
}

/* See the description in delta_api.h */
te_errno
delta_client_call(delta_client *cli, const char *request,
                  char *response, size_t response_len)
{
    te_errno rc;

    rc = delta_client_send(cli, request);
    if (rc != 0)
        return rc;

    rc = delta_client_recv(cli, response, response_len);
    if (rc != 0)
        return rc;

    RING("Delta API: %s -> %s", request, response);
    return 0;
}

/* See the description in delta_api.h */
bool
delta_response_is(const char *actual, const char *expected)
{
    return strcmp(actual, expected) == 0;
}

/* See the description in delta_api.h */
bool
delta_response_has(const char *actual, const char *substr)
{
    return strstr(actual, substr) != NULL;
}
