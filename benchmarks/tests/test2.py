import utils

utils.tc_cleanup()
utils.create_download_files(1, "100M")
utils.tc_add_download_all("tbf rate 10mbps burst 32kbit latency 100ms")

ftp_time_sec, out = utils.tool_download("ftp", "file_1")
print("ftp:")
if out:
    print(out)
print(f'{ftp_time_sec} s\n')

http3_time_sec, out = utils.tool_download("http3", "file_1")
print("http3:")
if out:
    print(out)
print(f'{http3_time_sec} s')
