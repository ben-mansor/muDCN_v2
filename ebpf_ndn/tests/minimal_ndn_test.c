#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <sys/resource.h>
#include <bpf/bpf.h>
#include <bpf/libbpf.h>
#include <net/if.h>

#define MAX_NAME_LEN 256

// NDN name structure - must match the one in ndn_xdp.c
struct ndn_name {
    char name[MAX_NAME_LEN];
    __u16 len;
};

// Statistics structure - must match the one in ndn_xdp.c
struct ndn_stats {
    __u64 interests_received;
    __u64 interests_forwarded;
    __u64 interests_dropped;
    __u64 data_received;
    __u64 data_forwarded;
    __u64 cache_hits;
    __u64 cache_misses;
};

int main(int argc, char **argv) {
    struct rlimit r = {RLIM_INFINITY, RLIM_INFINITY};
    int map_fd_stats, map_fd_cache;
    int prog_fd;
    int ifindex;
    char *interface;

    if (argc != 2) {
        printf("Usage: %s <interface>\n", argv[0]);
        return 1;
    }
    
    interface = argv[1];
    
    // Set resource limits for BPF
    if (setrlimit(RLIMIT_MEMLOCK, &r)) {
        perror("setrlimit failed");
        return 1;
    }

    // Find interface index
    ifindex = if_nametoindex(interface);
    if (ifindex == 0) {
        perror("Failed to find interface");
        return 1;
    }
    printf("Interface %s has ifindex %d\n", interface, ifindex);

    // Load BPF program from file
    struct bpf_object *obj;
    struct bpf_program *prog;
    
    obj = bpf_object__open_file("../build/ndn_xdp.o", NULL);
    if (libbpf_get_error(obj)) {
        fprintf(stderr, "Failed to open BPF object file\n");
        return 1;
    }

    // Find the XDP program within the object file
    prog = bpf_object__find_program_by_name(obj, "ndn_xdp");
    if (!prog) {
        fprintf(stderr, "Failed to find XDP program in object\n");
        bpf_object__close(obj);
        return 1;
    }

    // Set program type to XDP
    bpf_program__set_type(prog, BPF_PROG_TYPE_XDP);

    // Load the program
    if (bpf_object__load(obj)) {
        fprintf(stderr, "Failed to load BPF object\n");
        bpf_object__close(obj);
        return 1;
    }

    // Get file descriptor for the loaded program
    prog_fd = bpf_program__fd(prog);
    if (prog_fd < 0) {
        fprintf(stderr, "Failed to get program FD\n");
        bpf_object__close(obj);
        return 1;
    }

    // Find the maps
    map_fd_stats = bpf_object__find_map_fd_by_name(obj, "ndn_stats_map");
    if (map_fd_stats < 0) {
        fprintf(stderr, "Failed to find stats map\n");
    } else {
        printf("Found stats map: FD %d\n", map_fd_stats);
    }

    map_fd_cache = bpf_object__find_map_fd_by_name(obj, "ndn_name_cache");
    if (map_fd_cache < 0) {
        fprintf(stderr, "Failed to find cache map\n");
    } else {
        printf("Found cache map: FD %d\n", map_fd_cache);
    }

    // Attach XDP program to the interface
    int flags = 0; // Use XDP_FLAGS_SKB_MODE if needed
    if (bpf_set_link_xdp_fd(ifindex, prog_fd, flags) < 0) {
        fprintf(stderr, "Failed to attach XDP program to interface\n");
        bpf_object__close(obj);
        return 1;
    }

    printf("Successfully attached XDP program to %s\n", interface);
    printf("Press Ctrl+C to stop and detach...\n");

    // Monitor stats every second
    struct ndn_stats stats = {};
    int key = 0;

    while (1) {
        sleep(1);
        
        if (map_fd_stats >= 0) {
            if (bpf_map_lookup_elem(map_fd_stats, &key, &stats) == 0) {
                printf("\n--- NDN XDP Stats ---\n");
                printf("Interests received:  %llu\n", stats.interests_received);
                printf("Interests forwarded: %llu\n", stats.interests_forwarded);
                printf("Interests dropped:   %llu\n", stats.interests_dropped);
                printf("Data received:       %llu\n", stats.data_received);
                printf("Data forwarded:      %llu\n", stats.data_forwarded);
                printf("Cache hits:          %llu\n", stats.cache_hits);
                printf("Cache misses:        %llu\n", stats.cache_misses);
            } else {
                printf("Failed to read stats map\n");
            }
        }
    }

    // Cleanup (never reached due to the infinite loop, but included for completeness)
    bpf_set_link_xdp_fd(ifindex, -1, flags); // Detach program
    bpf_object__close(obj);
    
    return 0;
}
