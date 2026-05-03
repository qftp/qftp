import utils

utils.tc_cleanup()
utils.create_download_files(1, "100M")

ftp_time_sec, out = utils.tool_download("ftp", ["file_1"])
http3_time_sec, out = utils.tool_download("http3", ["file_1"])

print("file size,tool,time")
print(f'100M,ftp,{ftp_time_sec}s')
print(f'100M,http3,{http3_time_sec}s')
