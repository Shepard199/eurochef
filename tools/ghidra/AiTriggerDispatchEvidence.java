import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.Memory;

public class AiTriggerDispatchEvidence extends GhidraScript {
    private void dump(long raw, DecompInterface d) throws Exception {
        Address a=toAddr(raw); Function f=getFunctionContaining(a);
        println(String.format("\n=== 0x%08X ===", raw));
        if(f==null){println("NO FUNCTION");return;}
        println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
        DecompileResults r=d.decompileFunction(f,60,monitor);
        if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
    }
    @Override public void run() throws Exception {
        DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
        long[] ts={0x0047E4F0L,0x0047E8B0L,0x0047EA30L,0x0047EA70L,0x0042F440L};
        for(long t:ts) dump(t,d);
        println("\n=== TABLE 0x0061F380 (0..95) ===");
        for(int i=0;i<96;i++){
            long base=0x0061F380L+i*16L;
            println(String.format("%02d: %08X %08X %08X %08X",i,
                Integer.toUnsignedLong(getInt(toAddr(base))),Integer.toUnsignedLong(getInt(toAddr(base+4))),
                Integer.toUnsignedLong(getInt(toAddr(base+8))),Integer.toUnsignedLong(getInt(toAddr(base+12)))));
        }
        d.dispose();
    }
}
