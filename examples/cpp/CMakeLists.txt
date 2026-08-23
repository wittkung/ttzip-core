cmake_minimum_required(VERSION 3.20)
project(TTZipCppExample CXX)

set(CMAKE_CXX_STANDARD 20)

find_package(TTZip REQUIRED)

add_executable(ttzip_cpp_demo main.cpp)
target_link_libraries(ttzip_cpp_demo PRIVATE TTZip::Core)
