// Count distinct composed pixels within the test window. No screenshots are saved.
#include <windows.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>
#include <array>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <vector>
using Microsoft::WRL::ComPtr;
using Clock = std::chrono::steady_clock;

static void check(HRESULT hr, const char* operation) {
    if (FAILED(hr)) throw std::runtime_error(operation);
}

static int measure(HWND window) {
    SetProcessDPIAware();
    RECT client{};
    if (!GetClientRect(window, &client)) throw std::runtime_error("Missing client area");
    POINT point{client.right * 55 / 100, client.bottom / 3};
    ClientToScreen(window, &point);
    const HMONITOR screen = MonitorFromPoint(point, MONITOR_DEFAULTTONULL);
    ComPtr<IDXGIFactory1> factory;
    check(CreateDXGIFactory1(IID_PPV_ARGS(&factory)), "CreateDXGIFactory1");
    ComPtr<IDXGIAdapter1> adapter;
    ComPtr<IDXGIOutput> output;
    DXGI_OUTPUT_DESC desc{};
    for (UINT i = 0; !output; ++i) {
        if (factory->EnumAdapters1(i, adapter.ReleaseAndGetAddressOf()) != S_OK)
            throw std::runtime_error("Target monitor unavailable");
        for (UINT j = 0; adapter->EnumOutputs(j, output.ReleaseAndGetAddressOf()) == S_OK; ++j) {
            check(output->GetDesc(&desc), "GetDesc");
            if (desc.Monitor == screen) break;
            output.Reset();
        }
    }
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> context;
    check(D3D11CreateDevice(adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr, 0, nullptr, 0,
          D3D11_SDK_VERSION, &device, nullptr, &context), "D3D11CreateDevice");
    ComPtr<IDXGIOutput1> output1;
    ComPtr<IDXGIOutputDuplication> frames;
    check(output.As(&output1), "IDXGIOutput1");
    check(output1->DuplicateOutput(device.Get(), &frames), "DuplicateOutput");
    constexpr UINT width = 96, height = 240, stride = width * 4;
    POINT last{point.x + LONG(width) - 1, point.y + LONG(height) - 1};
    auto visible = [&] {
        return GetAncestor(WindowFromPoint(point), GA_ROOT) == window &&
               GetAncestor(WindowFromPoint(last), GA_ROOT) == window;
    };
    if (!visible() || !PtInRect(&desc.DesktopCoordinates, last))
        throw std::runtime_error("The test gallery must be visible on one monitor");
    const UINT x = point.x - desc.DesktopCoordinates.left, y = point.y - desc.DesktopCoordinates.top;
    D3D11_BOX region{x, y, 0, x + width, y + height, 1};
    D3D11_TEXTURE2D_DESC texture{};
    texture.Width = width;
    texture.Height = height;
    texture.MipLevels = texture.ArraySize = texture.SampleDesc.Count = 1;
    texture.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    texture.Usage = D3D11_USAGE_STAGING;
    texture.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    ComPtr<ID3D11Texture2D> sample;
    check(device->CreateTexture2D(&texture, nullptr, &sample), "CreateTexture2D");
    std::vector<unsigned char> previous(stride * height);
    std::array<int, 12> distinct{};
    std::vector<long long> timestamps;
    LARGE_INTEGER frequency{};
    QueryPerformanceFrequency(&frequency);
    puts("ready");
    fflush(stdout);
    auto start = Clock::now();
    bool havePrevious = false;
    for (;;) {
        const auto second = std::chrono::duration_cast<std::chrono::seconds>(Clock::now() - start).count();
        if (second >= distinct.size()) break;
        if (!visible()) throw std::runtime_error("The sampled region was covered");
        DXGI_OUTDUPL_FRAME_INFO info{};
        ComPtr<IDXGIResource> resource;
        HRESULT hr = frames->AcquireNextFrame(50, &info, &resource);
        if (hr == DXGI_ERROR_WAIT_TIMEOUT) continue;
        check(hr, "AcquireNextFrame");
        if (info.LastPresentTime.QuadPart) {
            ComPtr<ID3D11Texture2D> desktop;
            check(resource.As(&desktop), "Desktop texture");
            context->CopySubresourceRegion(sample.Get(), 0, 0, 0, 0, desktop.Get(), 0, &region);
            D3D11_MAPPED_SUBRESOURCE mapped{};
            check(context->Map(sample.Get(), 0, D3D11_MAP_READ, 0, &mapped), "Map");
            bool changed = false;
            for (UINT row = 0; row < height; ++row) {
                auto* src = static_cast<unsigned char*>(mapped.pData) + row * mapped.RowPitch;
                auto* dst = previous.data() + row * stride;
                changed |= memcmp(src, dst, stride) != 0;
                memcpy(dst, src, stride);
            }
            context->Unmap(sample.Get(), 0);
            if (havePrevious && changed) {
                ++distinct[second];
                if (second > 0 && second < 11) timestamps.push_back(info.LastPresentTime.QuadPart);
            }
            havePrevious = true;
        }
        check(frames->ReleaseFrame(), "ReleaseFrame");
    }
    for (auto count : distinct) printf("%d\n", count);
    for (size_t i = 1; i < timestamps.size(); ++i)
        printf("%.3f\n", (timestamps[i] - timestamps[i - 1]) * 1000.0 / frequency.QuadPart);
    return havePrevious ? 0 : 1;
}

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    try {
        return measure(reinterpret_cast<HWND>(static_cast<uintptr_t>(_strtoui64(argv[1], nullptr, 10))));
    } catch (const std::exception& error) {
        fprintf(stderr, "%s\n", error.what());
        return 1;
    }
}
