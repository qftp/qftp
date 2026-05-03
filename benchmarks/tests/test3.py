import utils

utils.tc_cleanup()
utils.create_download_files(2, "100M")

ftp_time_sec, out = utils.tool_download("ftp", ["file_1", "file_2"])
http3_time_sec, out = utils.tool_download("http3", ["file_1", "file_2"])

print("file count,tool,time")
print(f'2,ftp,{ftp_time_sec}s')
print(f'2,http3,{http3_time_sec}s')
