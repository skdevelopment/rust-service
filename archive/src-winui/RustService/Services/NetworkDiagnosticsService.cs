using System.Diagnostics;
using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;
using System.Text.RegularExpressions;
using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class NetworkDiagnosticsService
{
    public IReadOnlyList<NetworkInterfaceDetails> GetInterfaces()
    {
        var list = new List<NetworkInterfaceDetails>();
        foreach (var nic in NetworkInterface.GetAllNetworkInterfaces())
        {
            var props = nic.GetIPProperties();
            list.Add(new NetworkInterfaceDetails
            {
                Name = nic.Name,
                Description = nic.Description,
                MacAddress = nic.GetPhysicalAddress().ToString(),
                Ipv4Addresses = props.UnicastAddresses.Where(a => a.Address.AddressFamily == AddressFamily.InterNetwork).Select(a => a.Address.ToString()).ToList(),
                Ipv6Addresses = props.UnicastAddresses.Where(a => a.Address.AddressFamily == AddressFamily.InterNetworkV6).Select(a => a.Address.ToString()).ToList(),
                Gateway = props.GatewayAddresses.FirstOrDefault()?.Address?.ToString(),
                DnsServers = props.DnsAddresses.Select(a => a.ToString()).ToList(),
                Status = nic.OperationalStatus.ToString(),
                SpeedMbps = nic.Speed > 0 ? nic.Speed / 1_000_000 : null,
                InterfaceType = nic.NetworkInterfaceType.ToString()
            });
        }

        return list;
    }

    public async Task<PingResult> PingAsync(string host, int count = 4)
    {
        var result = new PingResult { Host = host, PacketsSent = count };
        try
        {
            var addresses = await Dns.GetHostAddressesAsync(host);
            result.ResolvedIp = addresses.FirstOrDefault()?.ToString();
            using var ping = new Ping();
            var times = new List<long>();
            for (var i = 1; i <= count; i++)
            {
                try
                {
                    var reply = await ping.SendPingAsync(host, 2000);
                    var ok = reply.Status == IPStatus.Success;
                    if (ok)
                    {
                        result.PacketsReceived++;
                        times.Add(reply.RoundtripTime);
                    }

                    result.Replies.Add(new Models.PingReply
                    {
                        Seq = i,
                        Ttl = reply.Options?.Ttl,
                        TimeMs = ok ? reply.RoundtripTime : null,
                        Status = reply.Status.ToString()
                    });
                }
                catch (Exception ex)
                {
                    result.Replies.Add(new Models.PingReply { Seq = i, Status = ex.Message });
                }
            }

            result.PacketLossPercent = count == 0 ? 0 : (count - result.PacketsReceived) * 100.0 / count;
            if (times.Count > 0)
            {
                result.MinMs = times.Min();
                result.MaxMs = times.Max();
                result.AvgMs = (long)times.Average();
            }
        }
        catch (Exception ex)
        {
            result.Error = ex.Message;
        }

        return result;
    }

    public async Task<TracerouteResult> TraceAsync(string host)
    {
        var result = new TracerouteResult { Host = host };
        try
        {
            result.ResolvedIp = (await Dns.GetHostAddressesAsync(host)).FirstOrDefault()?.ToString();
            var proc = await ProcessRunner.RunAsync("tracert", ["-d", "-h", "20", host], timeoutMs: 120_000);
            result.Error = proc.Success ? null : proc.Stderr.Trim();
            foreach (Match match in Regex.Matches(proc.Stdout, @"^\s*(\d+)\s+(.+)$", RegexOptions.Multiline))
            {
                var hop = int.Parse(match.Groups[1].Value);
                var rest = match.Groups[2].Value.Trim();
                var timedOut = rest.Contains('*');
                var ip = Regex.Match(rest, @"(\d+\.\d+\.\d+\.\d+)").Groups[1].Value;
                var times = Regex.Matches(rest, @"(\d+)\s*ms").Select(m => (long?)long.Parse(m.Groups[1].Value)).ToList();
                result.Hops.Add(new TracerouteHop
                {
                    HopNumber = hop,
                    IpAddress = string.IsNullOrEmpty(ip) ? null : ip,
                    Rtt1Ms = times.ElementAtOrDefault(0),
                    Rtt2Ms = times.ElementAtOrDefault(1),
                    Rtt3Ms = times.ElementAtOrDefault(2),
                    TimedOut = timedOut
                });
            }

            result.Completed = result.Hops.Any(h => h.IpAddress == result.ResolvedIp);
        }
        catch (Exception ex)
        {
            result.Error = ex.Message;
        }

        return result;
    }

    public async Task<DnsLookupResult> LookupAsync(string query, string type = "A")
    {
        var sw = Stopwatch.StartNew();
        var result = new DnsLookupResult { Query = query, QueryType = type };
        try
        {
            if (type.Equals("A", StringComparison.OrdinalIgnoreCase) || type.Equals("AAAA", StringComparison.OrdinalIgnoreCase))
            {
                var family = type.Equals("AAAA", StringComparison.OrdinalIgnoreCase) ? AddressFamily.InterNetworkV6 : AddressFamily.InterNetwork;
                foreach (var address in await Dns.GetHostAddressesAsync(query))
                {
                    if (address.AddressFamily != family && type is "A" or "AAAA") continue;
                    result.Answers.Add(new DnsRecord { RecordType = type, Value = address.ToString() });
                }
            }
            else
            {
                var nslookup = await ProcessRunner.RunAsync("nslookup", ["-type=" + type, query]);
                foreach (var line in nslookup.Stdout.Split('\n'))
                {
                    if (line.Contains('='))
                    {
                        result.Answers.Add(new DnsRecord { RecordType = type, Value = line.Trim() });
                    }
                }

                result.Error = nslookup.Success ? null : nslookup.Stderr.Trim();
            }
        }
        catch (Exception ex)
        {
            result.Error = ex.Message;
        }

        result.ResponseTimeMs = sw.ElapsedMilliseconds;
        return result;
    }

    public async Task<WifiInfo> GetWifiAsync()
    {
        var info = new WifiInfo();
        var result = await ProcessRunner.RunAsync("netsh", ["wlan", "show", "interfaces"]);
        if (!result.Success)
        {
            info.Error = result.Combined.Trim();
            return info;
        }

        string? Field(string name) =>
            Regex.Match(result.Stdout, $@"^\s*{Regex.Escape(name)}\s*:\s*(.+)$", RegexOptions.Multiline | RegexOptions.IgnoreCase)
                .Groups[1].Value.Trim() is { Length: > 0 } v ? v : null;

        info.Ssid = Field("SSID");
        info.Connected = !string.IsNullOrWhiteSpace(info.Ssid) && info.Ssid != "";
        info.Bssid = Field("BSSID");
        info.RadioType = Field("Radio type");
        info.Authentication = Field("Authentication");
        if (int.TryParse(Regex.Match(Field("Signal") ?? "", @"\d+").Value, out var signal)) info.SignalPercent = signal;
        if (int.TryParse(Field("Channel"), out var channel)) info.Channel = channel;
        if (double.TryParse(Field("Receive rate (Mbps)"), out var rx)) info.ReceiveRateMbps = rx;
        if (double.TryParse(Field("Transmit rate (Mbps)"), out var tx)) info.TransmitRateMbps = tx;
        return info;
    }
}
