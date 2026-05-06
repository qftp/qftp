import utils

utils.net_cleanup()
utils.tc_add_download_all("root cake bandwidth 1gbit rtt 20ms besteffort")

file_counts = [1, 2, 5, 8, 10, 15, 25, 50]
times = []
for count in file_counts:
    print(f'Testing {count} files...')
    utils.create_download_files(count, "100M")
    names = [f"file_{i}" for i in range(1, count + 1)]
    ftp_time_sec, out = utils.tool_download("ftp", names)
    print(out)
    http3_time_sec, out = utils.tool_download("http3", names)
    print(out)
    times.append({"ftp": ftp_time_sec, "http3": http3_time_sec})

print("file count,ftp(s),http3(s)")
for i, count in enumerate(file_counts):
    print(f'{count},{times[i]["ftp"]},{times[i]["http3"]}')
