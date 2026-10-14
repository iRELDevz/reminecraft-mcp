#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#ifdef _WIN32
#define EXPORT __declspec(dllexport)
#include <winsock2.h>
#include <ws2tcpip.h>
#pragma comment(lib, "ws2_32.lib")
typedef SOCKET sock_t;
#define CLOSESOCK closesocket
#else
#define EXPORT
#include <arpa/inet.h>
#include <netdb.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>
typedef int sock_t;
#define CLOSESOCK close
#define INVALID_SOCKET -1
#endif

static const uint8_t RAKNET_MAGIC[16] = {
    0x00, 0xFF, 0xFF, 0x00, 0xFE, 0xFE, 0xFE, 0xFE,
    0xFD, 0xFD, 0xFD, 0xFD, 0x12, 0x34, 0x56, 0x78
};

static void put_u64be(std::vector<uint8_t>& v, uint64_t x) {
    for (int i = 7; i >= 0; --i) v.push_back((uint8_t)((x >> (i * 8)) & 0xFF));
}

static uint64_t now_ms() {
#ifdef _WIN32
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    uint64_t t = ((uint64_t)ft.dwHighDateTime << 32) | ft.dwLowDateTime;
    return (t / 10000ULL) - 11644473600000ULL;
#else
    struct timeval tv;
    gettimeofday(&tv, nullptr);
    return (uint64_t)tv.tv_sec * 1000ULL + (uint64_t)tv.tv_usec / 1000ULL;
#endif
}

extern "C" EXPORT int32_t remine_bedrock_ping(const char* host, uint16_t port, char* out, int32_t out_len) {
    if (!host || !out || out_len <= 0) return -1;
#ifdef _WIN32
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) return -2;
#endif
    int32_t rc = -3;
    sock_t s = INVALID_SOCKET;
    struct addrinfo hints, *res = nullptr;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_DGRAM;
    char portstr[16];
    snprintf(portstr, sizeof(portstr), "%u", (unsigned)port);
    if (getaddrinfo(host, portstr, &hints, &res) != 0 || !res) {
#ifdef _WIN32
        WSACleanup();
#endif
        return -4;
    }
    s = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
    if (s == INVALID_SOCKET) {
        freeaddrinfo(res);
#ifdef _WIN32
        WSACleanup();
#endif
        return -5;
    }
#ifdef _WIN32
    DWORD tvo = 3000;
    setsockopt(s, SOL_SOCKET, SO_RCVTIMEO, (const char*)&tvo, sizeof(tvo));
#else
    struct timeval tvo;
    tvo.tv_sec = 3;
    tvo.tv_usec = 0;
    setsockopt(s, SOL_SOCKET, SO_RCVTIMEO, &tvo, sizeof(tvo));
#endif
    std::vector<uint8_t> pkt;
    pkt.push_back(0x01);
    put_u64be(pkt, now_ms());
    pkt.insert(pkt.end(), RAKNET_MAGIC, RAKNET_MAGIC + 16);
    put_u64be(pkt, 0x123456789ABCDEFULL);
    if (sendto(s, (const char*)pkt.data(), (int)pkt.size(), 0, res->ai_addr, (int)res->ai_addrlen) < 0) {
        rc = -6;
    } else {
        char buf[2048];
        int n = (int)recvfrom(s, buf, sizeof(buf), 0, nullptr, nullptr);
        if (n >= 35 && (uint8_t)buf[0] == 0x1C) {
            uint16_t len = ((uint8_t)buf[33] << 8) | (uint8_t)buf[34];
            int start = 35;
            int end = (len > 0 && start + len <= n) ? start + len : n;
            int copylen = end - start;
            if (copylen > out_len - 1) copylen = out_len - 1;
            memcpy(out, buf + start, copylen);
            out[copylen] = '\0';
            rc = copylen;
        } else {
            rc = -7;
        }
    }
    freeaddrinfo(res);
    CLOSESOCK(s);
#ifdef _WIN32
    WSACleanup();
#endif
    return rc;
}

#ifdef REMINE_BEDROCK_PING_CLI
int main(int argc, char** argv) {
    if (argc < 2) {
        printf("usage: bedrock_ping <host> [port]\n");
        return 1;
    }
    uint16_t port = argc >= 3 ? (uint16_t)atoi(argv[2]) : 19132;
    char out[1800];
    int32_t n = remine_bedrock_ping(argv[1], port, out, sizeof(out));
    if (n < 0) {
        printf("ping failed: %d\n", n);
        return 1;
    }
    printf("%s\n", out);
    return 0;
}
#endif
