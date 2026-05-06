import utils

utils.create_download_files(10, "10M")
names = [f"file_{i}" for i in range(1, 11)]
bandwidths = ["20mbit", "50mbit", "100mbit", "250mbit", "500mbit", "750mbit", "1gbit"]
times = []

for bandwidth in bandwidths:
    utils.net_cleanup()
    utils.tc_add_download_all(f"root cake bandwidth {bandwidth} rtt 20ms besteffort")

    print(f'Testing {bandwidth}...')
    ftp_time_sec, out = utils.tool_download("ftp", names)
    print(out)
    http3_time_sec, out = utils.tool_download("http3", names)
    print(out)
    times.append({"ftp": ftp_time_sec, "http3": http3_time_sec})

print("bandwidth,ftp(s),http3(s)")
for i, bandwidth in enumerate(bandwidths):
    print(f'{bandwidth},{times[i]["ftp"]},{times[i]["http3"]}')
