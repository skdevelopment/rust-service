using System.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Services;

namespace RustService.Views;

public sealed partial class NetworkPage : Page
{
    public NetworkPage()
    {
        InitializeComponent();
        Loaded += (_, _) => Ifaces_Click(this, new RoutedEventArgs());
    }

    private async void Ifaces_Click(object sender, RoutedEventArgs e)
    {
        var nics = Commands.GetDetailedNetworkInfo();
        Cards.ItemsSource = nics.Select(nic => new CardRow
        {
            Title = nic.Name,
            Subtitle = $"{nic.InterfaceType}  {string.Join(", ", nic.Ipv4Addresses)}",
            Meta = $"{nic.Description}  gw={nic.Gateway}",
            Badge = nic.Status
        }).ToList();
        var sb = new StringBuilder();
        foreach (var nic in nics)
        {
            sb.AppendLine($"{nic.Name}  {nic.Status}  {nic.InterfaceType}  {nic.SpeedMbps} Mbps");
            sb.AppendLine($"  {nic.Description}");
            sb.AppendLine($"  IPv4 {string.Join(", ", nic.Ipv4Addresses)}  gw={nic.Gateway}");
            sb.AppendLine($"  DNS {string.Join(", ", nic.DnsServers)}  MAC {nic.MacAddress}");
            sb.AppendLine();
        }

        var wifi = await Commands.GetWifiInfo();
        sb.AppendLine($"Wi-Fi: {(wifi.Connected ? wifi.Ssid : "not connected")}  {wifi.SignalPercent}%  {wifi.RadioType}  {wifi.Authentication}  {wifi.Error}");
        OverviewOut.Text = sb.ToString();
        WifiOut.Text = $"{wifi.Ssid} {wifi.SignalPercent}% {wifi.RadioType} {wifi.Authentication} {wifi.Bssid} {wifi.Error}";
    }

    private async void Ping_Click(object sender, RoutedEventArgs e)
    {
        var host = string.IsNullOrWhiteSpace(PingHost.Text) ? "google.com" : PingHost.Text;
        var count = (int)Math.Clamp(PingCount.Value, 1, 20);
        var r = await Commands.PingHost(host, count);
        PingOut.Text = $"{r.Host} {r.ResolvedIp} sent={r.PacketsSent} recv={r.PacketsReceived} loss={r.PacketLossPercent}% min={r.MinMs} avg={r.AvgMs} max={r.MaxMs}\r\n" +
                       string.Join("\r\n", r.Replies.Select(p => $"seq={p.Seq} {p.Status} {p.TimeMs}ms ttl={p.Ttl}")) +
                       (r.Error is null ? "" : "\r\n" + r.Error);
    }

    private async void Trace_Click(object sender, RoutedEventArgs e)
    {
        var host = string.IsNullOrWhiteSpace(TraceHost.Text) ? "google.com" : TraceHost.Text;
        var r = await Commands.TraceRoute(host);
        TraceOut.Text = $"{r.Host} {r.ResolvedIp} completed={r.Completed} {r.Error}\r\n" +
                        string.Join("\r\n", r.Hops.Select(h =>
                            $"{h.HopNumber,2}  {h.IpAddress} {h.Hostname}  {h.Rtt1Ms}/{h.Rtt2Ms}/{h.Rtt3Ms} ms{(h.TimedOut ? "  timeout" : "")}"));
    }

    private async void Dns_Click(object sender, RoutedEventArgs e)
    {
        var query = string.IsNullOrWhiteSpace(DnsHost.Text) ? "google.com" : DnsHost.Text;
        var type = (DnsType.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "A";
        var r = await Commands.DnsLookup(query, type);
        DnsOut.Text = $"{r.Query} {r.QueryType} {r.ResponseTimeMs}ms server={r.ServerUsed} {r.Error}\r\n" +
                      string.Join("\r\n", r.Answers.Select(a => $"{a.RecordType} {a.Value} ttl={a.Ttl}"));
    }

    private async void Wifi_Click(object sender, RoutedEventArgs e)
    {
        var w = await Commands.GetWifiInfo();
        WifiOut.Text = $"connected={w.Connected}  ssid={w.Ssid}  signal={w.SignalPercent}%  radio={w.RadioType}  auth={w.Authentication}  bssid={w.Bssid}\r\n{w.Error}";
    }
}
