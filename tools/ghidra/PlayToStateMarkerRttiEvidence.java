import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.symbol.Reference;

public class PlayToStateMarkerRttiEvidence extends GhidraScript {
    public void run() throws Exception {
        Memory m = currentProgram.getMemory();
        long[] addrs = {0x005DF8D4L,0x005DF8D0L,0x005DF8D8L,0x005DF8CCL};
        for (long raw: addrs) {
            Address a=toAddr(raw);
            println(String.format("=== %08X ===",raw));
            for(int i=-4;i<=12;i+=4){
                Address p=a.add(i);
                int v=m.getInt(p);
                println(String.format("%s %+d -> %08X",p,i,v));
            }
        }
        println("=== refs to 005DF8D4 ===");
        for (Reference r: getReferencesTo(toAddr(0x005DF8D4L))) println(r.toString());
    }
}
