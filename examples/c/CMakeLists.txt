cmake_minimum_required(VERSION 3.20)
project(TTZipCExample C)

set(CMAKE_C_STANDARD 11)

find_package(TTZip REQUIRED)

add_executable(ttzip_c_demo main.c)
target_link_libraries(ttzip_c_demo PRIVATE TTZip::Core)
