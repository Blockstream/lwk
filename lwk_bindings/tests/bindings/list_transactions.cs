// This is tested separetely, not tested with uniffi test-framework

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;

using uniffi.lwk;

class Program
{
    static void Main(string[] args)
    {
        Network network = Network.Testnet();
        Console.WriteLine(network);

        Mnemonic mnemonic = new Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        Console.WriteLine(mnemonic);

        ElectrumClient client = network.DefaultElectrumClient();

        client.Ping();

        Signer signer = new Signer(mnemonic, network);
        WolletDescriptor desc = signer.WpkhSlip77Descriptor();
        Console.WriteLine(desc);

        Wollet wollet = new Wollet(network, desc, null);

        Update update = client.FullScan(wollet)!;
        wollet.ApplyUpdate(update);

        List<WalletTx> txList = wollet.Transactions();

        Console.WriteLine("Transactions {0}:", txList.Count);

        foreach (WalletTx tx in txList) {
            Console.WriteLine(tx.Txid());
        }
    }
}

