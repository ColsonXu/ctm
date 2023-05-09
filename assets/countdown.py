import time

if __name__ == "__main__":
    seconds = 15
    while seconds > 0:
        m = seconds // 60
        s = seconds % 60
        print(m, ":", s, sep="")
        seconds -= 1
        time.sleep(1)
