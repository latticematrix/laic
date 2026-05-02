# LAIC Performance Evidence

This page summarizes version-marked performance evidence for LAIC releases.

## Versioning Rule

Performance evidence is release-scoped. Do not overwrite an older release's measured values when a later version improves or expands the evidence. Add a new version-marked section instead.

The numbers below belong to `0.1.0 MVP`. They are measured evidence from local validation rigs, same-LAN hosts, and one cloud VM WAN endpoint. They are not a production SLA, not a maximum-capacity claim, and not a promise for every deployment topology.

## 0.1.0 MVP

## Performance Advantages

For `0.1.0 MVP`, LAIC's measured advantage is that the mechanism layer can move AI-system messages with low overhead across four validated transport shapes:

- Local IPC stays in tens of microseconds on the current Windows local validation path after the receive-loop optimization.
- Localhost QUIC stays below 1 ms p95 in the current Windows local validation path.
- Same-LAN QUIC stays below 2.1 ms p95 across the validated fixed-count, 300s soak, and 4-client fan-out shapes.
- Public-WAN QUIC/mTLS to one cloud VM endpoint stays below 20 ms p95 across the validated two-host fixed-count, 300s soak, and 4-client fan-out shapes.

This supports a bounded `0.1.0 MVP` performance statement: LAIC has credible mechanism-layer transport evidence for fast local, LAN, and public-WAN communication between LLM-adjacent or agent-adjacent components. It does not claim production SLA, maximum fan-out, packet-loss tolerance, multi-region failover, or real hosted model workload performance.

## Evidence Basis

The raw validation packets were produced in the private development workspace before this public export. They are summarized here as public-safe release evidence; this repository intentionally does not include private reviewer logs, local machine paths, cloud account details, or transient experiment packets.

| Evidence area | Public status |
| --- | --- |
| Codec baseline | captured for `0.1.0 MVP` |
| Transport loopback baseline | validated for baseline capture |
| Windows local validation | current post-optimization local transport evidence |
| Same-LAN QUIC | validated fixed-count, 300s soak, and 4-client fan-out shapes |
| Public-WAN QUIC/mTLS | validated fixed-count, 300s soak, and 4-client fan-out shapes from two client hosts |
| Validation inventory | summarized for public release documentation |

## Codec Baseline

| Benchmark | mean_us | median_us | std_dev_us |
| --- | ---: | ---: | ---: |
| Arrow 768f32 encode | 4.466 | 4.250 | 0.708 |
| Arrow 768f32 decode | 2.701 | 2.631 | 0.216 |

## Same-Process / Same-Host Loopback

Payload matrix: 16 B, 1024 B, and 16384 B. Throughput counts two messages per roundtrip.

| Transport | Payload | p50_us | p95_us | p99_us | mean_us | messages_per_sec | bytes_per_sec |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| IPC loopback | 16 B | 6.500 | 6.500 | 6.600 | 6.457 | 304275.065 | 4868401.035 |
| QUIC loopback | 16 B | 56.000 | 69.400 | 70.100 | 57.893 | 34467.902 | 551486.428 |
| IPC loopback | 1024 B | 7.100 | 7.300 | 7.500 | 7.088 | 273410.800 | 279972658.920 |
| QUIC loopback | 1024 B | 70.900 | 83.200 | 94.500 | 67.769 | 29345.306 | 30049593.568 |
| IPC loopback | 16384 B | 8.200 | 8.400 | 8.500 | 8.252 | 238748.955 | 3911662886.475 |
| QUIC loopback | 16384 B | 234.000 | 240.200 | 259.800 | 232.885 | 8582.586 | 140617087.929 |

## Windows Local Validation

Payload: 65536 B.

| Path | Shape | p50_us | p95_us | p99_us | messages_per_sec | bytes_per_sec | roundtrips | errors |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Cross-process IPC | single client | 37.500 | 43.900 | 48.000 | 46307.016 | 3034776568.650 | 200 | 0 |
| Localhost QUIC | single client | 611.900 | 792.300 | 1021.500 | 3099.471 | 203126900.847 | 200 | 0 |
| IPC fan-out | 8 clients, 100 rounds/client | 21.100 | 23.800 | 28.900 | 89925.025 | 5893326439.081 | 800 | 0 |
| Localhost QUIC fan-out | 8 clients, 100 rounds/client | 530.200 | 642.400 | 721.600 | 3709.167 | 243083968.354 | 800 | 0 |
| IPC soak | 300 s | 20.700 | 26.200 | 27.800 | 90750.690 | 5947437224.186 | 13612604 | 0 |
| Localhost QUIC soak | 300 s | 424.200 | 457.900 | 542.300 | 4614.749 | 302432201.357 | 692213 | 0 |

## Same-LAN QUIC

| Path | Shape | Payload | p50_us | p95_us | p99_us | messages_per_sec | bytes_per_sec | roundtrips | errors |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Same-LAN QUIC | fixed-count, 200 rounds | 1024 B | 425.000 | 580.900 | 678.000 | 4689.590 | 4802140.329 | 200 | 0 |
| Same-LAN QUIC | 300s soak | 65536 B | 1878.800 | 2076.400 | 2145.500 | 1064.734 | 69778425.930 | 159711 | 0 |
| Same-LAN QUIC | 4 clients, 50 rounds/client | 1024 B | 681.800 | 1246.700 | 1292.900 | 10790.513 | 11049485.293 | 200 | 0 |

## Public-WAN QUIC/mTLS

Endpoint shape: one cloud VM endpoint. The validated fan-out shape is 4 clients and 50 measured rounds per client.

| Path | Shape | Payload | p50_us | p95_us | p99_us | messages_per_sec | bytes_per_sec | roundtrips | errors |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Host 1 to cloud VM | fixed-count, 200 rounds | 1024 B | 17513.600 | 17722.900 | 17856.300 | 114.242 | 116984.134 | 200 | 0 |
| Host 2 to cloud VM | fixed-count, 200 rounds | 1024 B | 17782.600 | 18278.700 | 18948.000 | 112.513 | 115213.158 | 200 | 0 |
| Host 1 to cloud VM | 300s soak | 1024 B | 17520.100 | 17941.200 | 19286.700 | 113.491 | 116215.064 | 17024 | 0 |
| Host 2 to cloud VM | 300s soak | 1024 B | 13919.000 | 14453.500 | 17662.100 | 141.895 | 145300.588 | 21285 | 0 |
| Host 1 to cloud VM | 4 clients, 50 rounds/client | 1024 B | 18185.600 | 18566.800 | 18775.400 | 438.180 | 448695.857 | 200 | 0 |
| Host 2 to cloud VM | 4 clients, 50 rounds/client | 1024 B | 19025.200 | 19646.500 | 19723.200 | 420.640 | 430735.094 | 200 | 0 |

## Boundaries

The current MVP evidence does not claim:

- production SLA
- maximum fan-out capacity
- public-WAN fan-out above the reviewed 4-client shape
- long soak beyond the reviewed 300s windows
- packet-loss, jitter, degraded-network, or chaos tolerance
- multi-endpoint, multi-region, failover, relay, or multi-hop behavior
- real hosted LLM / agent workload end-to-end performance

An 8-client WAN fan-out experiment was deliberately not promoted into the `0.1.0 MVP` evidence set because it depended on a temporary local manifest variant. Treat the validated public-WAN fan-out claim as limited to the 4-client shape above.
