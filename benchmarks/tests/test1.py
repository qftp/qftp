import utils

utils.create_download_files(1, "100M")
ftp_time_sec = utils.tool_download("ftp", "file_1")
http3_time_sec = utils.tool_download("http3", "file_1")

print(f'ftp: {ftp_time_sec} s')
print(f'http3: {http3_time_sec} s')
