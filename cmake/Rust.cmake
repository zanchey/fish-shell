if(EXISTS "${CMAKE_SOURCE_DIR}/corrosion-vendor/")
    add_subdirectory("${CMAKE_SOURCE_DIR}/corrosion-vendor/")
else()
    include(FetchContent)

    # Don't let Corrosion's tests interfere with ours.
    set(CORROSION_TESTS OFF CACHE BOOL "" FORCE)

    FetchContent_Declare(
        Corrosion
        GIT_REPOSITORY https://github.com/mqudsi/corrosion
        GIT_TAG fish
    )

    FetchContent_MakeAvailable(Corrosion)

    add_custom_target(corrosion-vendor.tar.gz
        COMMAND git archive --format tar.gz --output "${CMAKE_BINARY_DIR}/corrosion-vendor.tar.gz"
        --prefix corrosion-vendor/ HEAD
        WORKING_DIRECTORY ${corrosion_SOURCE_DIR}
    )
endif()

set(fish_rust_target "fish")

set(FISH_CRATE_FEATURES)
if(NOT DEFINED CARGO_FLAGS)
    # Corrosion doesn't like an empty string as FLAGS. This is basically a no-op alternative.
    # See https://github.com/corrosion-rs/corrosion/issues/356
    set(CARGO_FLAGS "--config" "foo=0")
endif()
if(DEFINED ASAN)
    list(APPEND CARGO_FLAGS "-Z" "build-std")
    list(APPEND FISH_CRATE_FEATURES FEATURES "asan")
endif()

corrosion_import_crate(
    MANIFEST_PATH "${CMAKE_SOURCE_DIR}/Cargo.toml"
    CRATES "fish"
    "${FISH_CRATE_FEATURES}"
    FLAGS "${CARGO_FLAGS}"
)

# Temporary hack to propogate CMake flags/options to build.rs. We need to get CMake to evaluate the
# truthiness of the strings if they are set.
set(CMAKE_WITH_GETTEXT "1")
if(DEFINED WITH_GETTEXT AND NOT "${WITH_GETTEXT}")
    set(CMAKE_WITH_GETTEXT "0")
endif()

# CMAKE_BINARY_DIR can include symlinks, since we want to compare this to the dir fish is executed in we need to canonicalize it.
file(REAL_PATH "${CMAKE_BINARY_DIR}" fish_binary_dir)
