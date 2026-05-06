import utils

utils.tc_cleanup()
utils.tc_add_download_all("root netem delay 10ms")

file_counts = [1, 2, 5, 10, 25, 50, 100, 150, 200, 250, 500]
times = []
for count in file_counts:
    print(f'Testing {count} files...')
    utils.create_download_files(count, "10M")
    ftp_time_sec, _ = utils.tool_download("ftp", ["file_1"])
    http3_time_sec, _ = utils.tool_download("http3", ["file_1"])
    times.append({"ftp": ftp_time_sec, "http3": http3_time_sec})

print("file count,ftp(s),http3(s)")
for i, count in enumerate(file_counts):
    print(f'{count},{times[i]["ftp"]},{times[i]["http3"]}')
