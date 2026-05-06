import utils

utils.create_download_files(10, "10M")
names = [f"file_{i}" for i in range(1, 11)]
rtts = ["100us", "1ms", "10ms", "30ms", "100ms", "300ms", "10s"]
times = []

for rtt in rtts:
    utils.net_cleanup()
    utils.tc_add_download_all(f"root cake bandwidth 1gbit rtt {rtt}")

    print(f'Testing {rtt}...')
    ftp_time_sec, out = utils.tool_download("ftp", names)
    print(out)
    http3_time_sec, out = utils.tool_download("http3", names)
    print(out)
    times.append({"ftp": ftp_time_sec, "http3": http3_time_sec})

print("bandwidth,ftp(s),http3(s)")
for i, rtt in enumerate(rtts):
    print(f'{rtt},{times[i]["ftp"]},{times[i]["http3"]}')
