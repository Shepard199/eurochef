import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
public class WatchBotOwner150Evidence extends GhidraScript {
 void dc(long a, DecompInterface d)throws Exception { Address x=toAddr(a); disassemble(x); Function f=getFunctionContaining(x); println("\n=== "+x+" "+(f==null?"<missing>":f.getName())+" ==="); if(f!=null){DecompileResults r=d.decompileFunction(f,120,monitor); if(r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());}}
 public void run()throws Exception{ DecompInterface d=new DecompInterface(); d.openProgram(currentProgram); long[] a={0x00493800L,0x00495ED0L,0x00496C60L,0x00490C10L,0x00490850L}; for(long x:a)dc(x,d); d.dispose(); }
}
