set -l arping_version $(arping -V 2>&1)

if string match -eiq -- iputils $arping_version
    # this is for arping from iputils
    # https://github.com/iputils/iputils
    complete -c arping -xa "(__fish_print_hostnames)"
    complete -c arping -s A -d "Unsolicited ARP mode (ARP REPLY)"
    complete -c arping -s b -d "Send only MAC level broadcasts"
    complete -c arping -s c -x -d "Stop after number of packets"
    complete -c arping -s D -d "Duplicate address detection mode"
    complete -c arping -s f -d "Finish after the first reply"
    complete -c arping -s I -xa "(__fish_print_interfaces)" -d "Network device where to send packets"
    complete -c arping -s h -d "Print help page"
    complete -c arping -s q -d "Quiet output"
    complete -c arping -s s -xa "(__fish_print_addresses | grep -v :)" -d "IP source address to use in ARP packets"
    complete -c arping -s U -d "Unsolicited ARP mode (ARP REQUEST)"
    complete -c arping -s V -d "Print version"
    complete -c arping -s w -x -d "Specify a timeout (seconds)"
    complete -c arping -s i -x -d "Specify an interval between packets (seconds)"

else if string match -eiq -- habets $arping_version
    # this is for arping from Thomas Habets
    # https://github.com/ThomasHabets/arping
    complete -c arping -xa "(__fish_print_hostnames)"
    complete -c arping -l help -d "Show extended help"
    complete -c arping -s 0 -d "Ping with source IP address 0.0.0.0"
    complete -c arping -s a -d "Audible ping"
    complete -c arping -s A -d "Only count addresses matching requested address"
    complete -c arping -s b -d "Ping with source IP address 255.255.255.255"
    complete -c arping -s B -d "Use instead of host if you want to address 255.255.255.255"
    complete -c arping -s c -x -d "Only send count requests"
    complete -c arping -s C -x -d "Only wait for count replies"
    complete -c arping -s d -d "Find duplicate replies"
    complete -c arping -s D -d "Display answers as exclamation points and missing packets as dots"
    complete -c arping -s e -d "Audible ping, but beep when there is no reply"
    complete -c arping -s F -d "Don't try to be smart about the interface name"
    complete -c arping -s f -xa "(__fish_print_groups)" -d "setgid() to this group instead of the nobody group"
    complete -c arping -s h -d "Displays a help message"
    complete -c arping -s i -xa "(__fish_print_interfaces)" -d "Use the specified interface"
    complete -c arping -s m -x -d "Type of timestamp to use for incoming packets"
    complete -c arping -s p -d "Turn on promiscious mode on interface"
    complete -c arping -s P -d "Send ARP replies instead of requests"
    complete -c arping -s q -d "Do not display messages, except errors"
    complete -c arping -s Q -x -d "802.1p priority to set"
    complete -c arping -s r -d "Raw output: only the MAC/IP address is displayed"
    complete -c arping -s R -d "Raw output: Like -r but shows the other one"
    complete -c arping -s s -x -d "Set source MAC address"
    complete -c arping -s S -xa "(__fish_print_addresses | grep -v :)" -d "Ping with this source IP address"
    complete -c arping -s t -x -d "Set target MAC address"
    complete -c arping -s T -x -d "Set target address when pinging MACs that won't respond to broadcasts"
    complete -c arping -s u -d "Show index=received/sent"
    complete -c arping -s U -d "Send unsolicited ARP"
    complete -c arping -s v -d "Verbose output"
    complete -c arping -s V -x -d "VLAN tag to set"
    complete -c arping -s w -x -d "Timeout before ping exits (seconds)"
    complete -c arping -s W -x -d "Time to wait between pings (seconds)"
    complete -c arping -s z -d "Enable seccomp"
    complete -c arping -s z -d "Disable seccomp"
end