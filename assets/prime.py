import math

if __name__ == "__main__":
    count = 3
    while count < 2000000:
        is_prime = True
        for x in range(2, int(math.sqrt(count) + 1)):
            if count % x == 0: 
                is_prime = False
                break
        if is_prime:
            print(count)
        count += 1