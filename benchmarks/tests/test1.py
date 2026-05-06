import utils

utils.tc_cleanup()
utils.tc_add_download_all("root netem delay 10ms")

file_sizes = ["1M", "10M", "50M", "100M", "250M", "500M", "1G", "2G", "5G"]
times = []
for size in file_sizes:
    print(f'Testing {size}...')
    utils.create_download_files(1, size)
    print("Generated files")
    ftp_time_sec, _ = utils.tool_download("ftp", ["file_1"])
    print("ftp done")
    http3_time_sec, _ = utils.tool_download("http3", ["file_1"])
    print("http3 done")
    times.append({"ftp": ftp_time_sec, "http3": http3_time_sec})

print("file size,ftp(s),http3(s)")
for i, size in enumerate(file_sizes):
    print(f'{size},{times[i]["ftp"]},{times[i]["http3"]}')
