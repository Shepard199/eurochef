import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import java.util.*;

public class Field34WriteCensusEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  InstructionIterator it=currentProgram.getListing().getInstructions(true);
  Map<Long,Integer> counts=new LinkedHashMap<>();
  int total=0;
  while(it.hasNext()){
    Instruction ins=it.next();
    String s=ins.toString().toLowerCase();
    if(!(s.startsWith("mov ")||s.startsWith("lea ")||s.startsWith("xchg ")||s.startsWith("and ")||s.startsWith("or "))) continue;
    if(!(s.contains("+ 0x34]")||s.contains("+0x34]"))) continue;
    Function f=getFunctionContaining(ins.getAddress());
    long e=f==null?0:f.getEntryPoint().getOffset();
    counts.put(e,counts.getOrDefault(e,0)+1);
    println(ins.getAddress()+"  "+ins+"  FUNC="+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
    total++;
  }
  println("TOTAL="+total+" FUNCTIONS="+counts.size());
 }
}